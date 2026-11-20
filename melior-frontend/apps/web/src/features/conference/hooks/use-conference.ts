import { useCallback, useEffect, useReducer, useRef, useState } from 'react';
import {
    canShareScreen,
    captureInput,
    captureLocalMedia,
    captureScreen,
    createPeerSession,
    createSignallingClient,
    isScreenShareCancelled,
    loadCallPreferences,
    loadDevicePreferences,
    type PeerSession,
    type SignallingClient,
    saveCallPreferences,
    saveDevicePreference,
    stopMedia,
} from '@/features/conference/api';
import {
    type CallPreferences,
    type ChannelId,
    type CloseDecision,
    type CloseReason,
    type ConferenceState,
    conferenceReducer,
    createInitialConferenceState,
    decideAfterDiagnosis,
    decideOnClose,
    nextAttempt,
    type RoomProbe,
    reconnectDelayMs,
    type ServerMessage,
    type SessionProbe,
    type StopReason,
} from '@/features/conference/domain';

/** What the room says in chat: a message for everyone, or a refusal of the one this client sent. */
export type ChatEvent = Extract<ServerMessage, { channel: 'chat' | 'chat_error' }>;

type UseConferenceReturn = ConferenceState<MediaStream> & {
    /** Says something to the room. Returns false when there is no open connection, so nothing was sent. */
    sendChat: (text: string, clientId: string) => boolean;
    /** Whether this browser can share a screen at all. */
    screenShareSupported: boolean;
    /** Whether this participant is sharing its screen right now. */
    screenSharing: boolean;
    /** Why the last attempt to share failed; null after a success or when the person cancelled. */
    screenError: string | null;
    /** Asks which screen to share and starts sending it. Does nothing while sharing or not connected. */
    startScreenShare: () => Promise<void>;
    stopScreenShare: () => void;
    /** The microphone and camera in use, by device id. Empty until the camera is open. */
    activeDevices: { audioinput: string | null; videoinput: string | null };
    /** Switches to another microphone or camera during the call. Rejects when the device cannot be opened. */
    changeInput: (kind: 'audioinput' | 'videoinput', deviceId: string) => Promise<void>;
    /** Whether your microphone is muted: the others receive silence. Remembered for the next call. */
    microphoneMuted: boolean;
    /**
     * Whether your camera is off: the others receive black frames. The camera itself stays open, so its
     * indicator light stays on. Remembered for the next call.
     */
    cameraOff: boolean;
    setMicrophoneMuted: (muted: boolean) => void;
    setCameraOff: (off: boolean) => void;
};

type UseConferenceOptions = {
    /** Route all media through the TURN relay (diagnostics). */
    forceRelay?: boolean;
    /**
     * Re-checks the login session after a lost connection (the browser reports no status for a refused
     * upgrade). The caller runs the login gate on 401 and shows the no-access state on 403.
     */
    checkSession?: () => Promise<SessionProbe>;
    /** Asks whether the user may join this room (a 403 means "not invited"). */
    checkRoomAccess?: () => Promise<RoomProbe>;
    /** Called for every chat message the room receives and every refusal of this client's messages. */
    onChat?: (event: ChatEvent) => void;
};

/** Turns the microphone and camera tracks on or off as the person chose. Off means silence or black frames. */
const applyCallPreferences = (stream: MediaStream, preferences: CallPreferences) => {
    for (const track of stream.getAudioTracks()) track.enabled = !preferences.microphoneMuted;
    for (const track of stream.getVideoTracks()) track.enabled = !preferences.cameraOff;
};

const devicesOf = (stream: MediaStream): UseConferenceReturn['activeDevices'] => ({
    audioinput: stream.getAudioTracks()[0]?.getSettings().deviceId ?? null,
    videoinput: stream.getVideoTracks()[0]?.getSettings().deviceId ?? null,
});

/**
 * Orchestrates one conference: camera once, then a signalling socket plus peer connection per
 * connection, replaced automatically after an unexpected loss. The decisions (what a close means, how
 * long to wait) are pure functions in the domain; this hook only executes them.
 */
export const useConference = (channelId: ChannelId, options: UseConferenceOptions = {}): UseConferenceReturn => {
    const { forceRelay = false, checkSession, checkRoomAccess, onChat } = options;
    const checkSessionRef = useRef(checkSession);
    const checkRoomAccessRef = useRef(checkRoomAccess);
    const onChatRef = useRef(onChat);
    // The open connection, for the use cases that send outside the connection's own effect.
    const signallingRef = useRef<SignallingClient | null>(null);
    const peerRef = useRef<PeerSession | null>(null);
    // The captured screen outlives a connection: after a reconnect it is offered again.
    const screenRef = useRef<MediaStream | null>(null);
    const localRef = useRef<MediaStream | null>(null);
    const [callPreferences, setCallPreferences] = useState(loadCallPreferences);
    // Read by the capture and by input changes, which must not wait for a render to apply the switches.
    const callPreferencesRef = useRef(callPreferences);
    callPreferencesRef.current = callPreferences;
    const [activeDevices, setActiveDevices] = useState<UseConferenceReturn['activeDevices']>({
        audioinput: null,
        videoinput: null,
    });
    const [screenSharing, setScreenSharing] = useState(false);
    const [screenError, setScreenError] = useState<string | null>(null);
    useEffect(() => {
        checkSessionRef.current = checkSession;
        checkRoomAccessRef.current = checkRoomAccess;
        onChatRef.current = onChat;
    });
    const [state, dispatch] = useReducer(
        conferenceReducer<MediaStream>,
        undefined,
        createInitialConferenceState<MediaStream>,
    );

    useEffect(() => {
        let disposed = false;
        let signalling: SignallingClient | null = null;
        let peer: PeerSession | null = null;
        let local: MediaStream | null = null;
        let timer: ReturnType<typeof setTimeout> | null = null;
        // Bumped on every teardown so late callbacks of an old connection are ignored.
        let generation = 0;
        let attempt = 0;
        let reachedConnected = false;

        dispatch({ type: 'reset' });

        const release = () => {
            signalling?.close();
            peer?.close();
            signalling = null;
            signallingRef.current = null;
            peer = null;
            peerRef.current = null;
        };

        const teardown = () => {
            generation += 1;
            release();
        };

        const fail = (message: string) => {
            if (disposed) return;
            console.error(message);
            dispatch({ type: 'failed', message });
        };

        const stop = (reason: StopReason) => {
            teardown();
            dispatch({ type: 'stopped', reason });
        };

        const scheduleReconnect = (immediate: boolean) => {
            if (disposed) return;
            teardown();
            attempt = nextAttempt(attempt, reachedConnected);
            reachedConnected = false;
            dispatch({ type: 'reconnecting', attempt });
            const delay = immediate ? 0 : reconnectDelayMs(attempt, Math.random());
            if (timer) clearTimeout(timer);
            timer = setTimeout(connect, delay);
        };

        const probe = async <T>(check: (() => Promise<T>) | undefined, fallback: T): Promise<T> => {
            try {
                return check ? await check() : fallback;
            } catch (error) {
                console.warn('re-check failed', error);
                return fallback;
            }
        };

        const diagnose = async (checkRoom: boolean) => {
            const mine = generation;
            const session = await probe<SessionProbe>(checkSessionRef.current, 'error');
            const room =
                session === 'ok' && checkRoom
                    ? await probe<RoomProbe>(checkRoomAccessRef.current, 'unknown')
                    : 'unknown';
            if (disposed || mine !== generation) return;
            execute(decideAfterDiagnosis(session, room));
        };

        const execute = (decision: CloseDecision) => {
            if (disposed) return;
            switch (decision.kind) {
                case 'reconnect':
                    scheduleReconnect(decision.immediate);
                    break;
                case 'stop':
                    stop(decision.reason);
                    break;
                case 'diagnose':
                    void diagnose(decision.checkRoom);
                    break;
            }
        };

        const onClosed = (serverReason: CloseReason | null, opened: boolean) => {
            if (disposed) return;
            const decision = decideOnClose({ serverReason, opened });
            // The generation stays, so a diagnosis started now is still current when it finishes.
            release();
            execute(decision);
        };

        const handle = async (message: ServerMessage, mine: number) => {
            if (disposed || mine !== generation) return;
            switch (message.channel) {
                case 'session': {
                    if (peer) {
                        console.warn('duplicate session message ignored');
                        return;
                    }
                    dispatch({
                        type: 'session_started',
                        sessionId: message.payload.session_id,
                        username: message.payload.username,
                        participants: message.payload.participants,
                    });
                    const session = createPeerSession({
                        iceServers: message.payload.ice_servers,
                        forceRelay,
                        localStream: local,
                        onIceCandidate: (candidate) => {
                            if (mine === generation) signalling?.send({ channel: 'ice', payload: candidate });
                        },
                        onOffer: (offer) => {
                            if (mine === generation)
                                signalling?.send({ channel: 'sdp_offer', payload: { type: 'offer', sdp: offer } });
                        },
                        onRemoteMedia: (streams) => {
                            if (mine === generation) dispatch({ type: 'remote_media_updated', streams });
                        },
                        onConnected: () => {
                            if (disposed || mine !== generation) return;
                            reachedConnected = true;
                            dispatch({ type: 'connected' });
                            // A screen shared before a reconnect goes on being shared.
                            if (screenRef.current) session.shareScreen(screenRef.current);
                        },
                        onFailed: () => {
                            if (disposed || mine !== generation) return;
                            console.warn('peer connection failed, reconnecting');
                            scheduleReconnect(false);
                        },
                    });
                    peer = session;
                    peerRef.current = session;
                    const sdp = await session.createOffer();
                    if (disposed || mine !== generation) return;
                    signalling?.send({ channel: 'sdp_offer', payload: { type: 'offer', sdp } });
                    break;
                }
                case 'sdp_answer':
                    await peer?.applyAnswer(message.payload.sdp);
                    break;
                case 'sdp_offer': {
                    if (!peer) return;
                    const sdp = await peer.answerOffer(message.payload.sdp, message.payload.tracks);
                    if (disposed || mine !== generation) return;
                    signalling?.send({ channel: 'sdp_answer', payload: { type: 'answer', sdp } });
                    break;
                }
                case 'ice':
                    await peer?.addIceCandidate(message.payload);
                    break;
                case 'peer_joined':
                    dispatch({
                        type: 'peer_joined',
                        sessionId: message.payload.session_id,
                        username: message.payload.username,
                    });
                    break;
                case 'peer_left':
                    dispatch({ type: 'peer_left', sessionId: message.payload.session_id });
                    break;
                case 'chat':
                case 'chat_error':
                    onChatRef.current?.(message);
                    break;
                case 'closed':
                    // The socket is about to close; act now instead of waiting for the close event.
                    onClosed(message.payload.reason, true);
                    break;
                case 'error':
                    teardown();
                    fail(`Server error: ${message.payload.message}`);
                    break;
            }
        };

        function connect() {
            if (disposed) return;
            timer = null;
            const mine = generation;
            signalling = createSignallingClient({
                channelId,
                onMessage: (message) => handle(message, mine),
                onClose: ({ opened }) => {
                    if (disposed || mine !== generation) return;
                    onClosed(null, opened);
                },
            });
            signallingRef.current = signalling;
            dispatch({ type: 'socket_connecting' });
        }

        const start = async () => {
            try {
                const stream = await captureLocalMedia(loadDevicePreferences());
                if (disposed) {
                    stopMedia(stream);
                    return;
                }
                local = stream;
                localRef.current = stream;
                applyCallPreferences(stream, callPreferencesRef.current);
                setActiveDevices(devicesOf(stream));
                dispatch({ type: 'local_stream_ready', stream });
                connect();
            } catch (error) {
                fail(`Could not access the camera: ${error instanceof Error ? error.message : String(error)}`);
            }
        };

        void start();

        return () => {
            disposed = true;
            if (timer) clearTimeout(timer);
            teardown();
            if (local) stopMedia(local);
            localRef.current = null;
            if (screenRef.current) stopMedia(screenRef.current);
            screenRef.current = null;
            setScreenSharing(false);
        };
    }, [channelId, forceRelay]);

    const sendChat = useCallback(
        (text: string, clientId: string) =>
            signallingRef.current?.send({ channel: 'chat_send', payload: { text, client_id: clientId } }) ?? false,
        [],
    );

    const stopScreenShare = useCallback(() => {
        peerRef.current?.stopScreen();
        if (screenRef.current) stopMedia(screenRef.current);
        screenRef.current = null;
        setScreenSharing(false);
    }, []);

    const startScreenShare = useCallback(async () => {
        const peer = peerRef.current;
        if (!peer || screenRef.current) return;
        setScreenError(null);
        try {
            const screen = await captureScreen();
            // The connection may have gone while the person was choosing.
            if (peerRef.current !== peer) {
                stopMedia(screen);
                return;
            }
            screenRef.current = screen;
            // The browser's own "Stop sharing" button ends the track.
            screen.getVideoTracks()[0]?.addEventListener('ended', () => {
                if (screenRef.current === screen) stopScreenShare();
            });
            peer.shareScreen(screen);
            setScreenSharing(true);
        } catch (error) {
            if (isScreenShareCancelled(error)) return;
            setScreenError(`Could not share your screen: ${error instanceof Error ? error.message : String(error)}`);
        }
    }, [stopScreenShare]);

    const changeInput = useCallback(async (kind: 'audioinput' | 'videoinput', deviceId: string) => {
        const stream = localRef.current;
        if (!stream) return;
        const track = await captureInput(kind, deviceId);
        const old = kind === 'audioinput' ? stream.getAudioTracks()[0] : stream.getVideoTracks()[0];
        try {
            await peerRef.current?.replaceInput(track);
        } catch (error) {
            track.stop();
            throw error;
        }
        if (old) {
            stream.removeTrack(old);
            old.stop();
        }
        stream.addTrack(track);
        applyCallPreferences(stream, callPreferencesRef.current);
        saveDevicePreference(kind, deviceId);
        setActiveDevices(devicesOf(stream));
    }, []);

    const updateCallPreferences = useCallback((change: Partial<CallPreferences>) => {
        const next = { ...callPreferencesRef.current, ...change };
        callPreferencesRef.current = next;
        setCallPreferences(next);
        saveCallPreferences(next);
        if (localRef.current) applyCallPreferences(localRef.current, next);
    }, []);
    const setMicrophoneMuted = useCallback(
        (muted: boolean) => updateCallPreferences({ microphoneMuted: muted }),
        [updateCallPreferences],
    );
    const setCameraOff = useCallback(
        (off: boolean) => updateCallPreferences({ cameraOff: off }),
        [updateCallPreferences],
    );

    return {
        ...state,
        sendChat,
        microphoneMuted: callPreferences.microphoneMuted,
        cameraOff: callPreferences.cameraOff,
        setMicrophoneMuted,
        setCameraOff,
        activeDevices,
        changeInput,
        screenShareSupported: canShareScreen(),
        screenSharing,
        screenError,
        startScreenShare,
        stopScreenShare,
    };
};
