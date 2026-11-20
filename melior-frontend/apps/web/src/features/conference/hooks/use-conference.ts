import { useEffect, useReducer, useRef } from 'react';
import {
    captureLocalMedia,
    createPeerSession,
    createSignallingClient,
    type Mesh,
    type PeerSession,
    type SignallingClient,
    stopMedia,
} from '@/features/conference/api';
import {
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
import { startMesh } from './start-mesh';

type UseConferenceReturn = ConferenceState<MediaStream>;

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
};

/**
 * Orchestrates one conference: camera once, then a signalling socket plus peer connection per
 * connection, replaced automatically after an unexpected loss. The decisions (what a close means, how
 * long to wait) are pure functions in the domain; this hook only executes them.
 */
export const useConference = (channelId: ChannelId, options: UseConferenceOptions = {}): UseConferenceReturn => {
    const { forceRelay = false, checkSession, checkRoomAccess } = options;
    const checkSessionRef = useRef(checkSession);
    const checkRoomAccessRef = useRef(checkRoomAccess);
    useEffect(() => {
        checkSessionRef.current = checkSession;
        checkRoomAccessRef.current = checkRoomAccess;
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
        let mesh: Mesh | null = null;
        let local: MediaStream | null = null;
        let timer: ReturnType<typeof setTimeout> | null = null;
        // Bumped on every teardown so late callbacks of an old connection are ignored.
        let generation = 0;
        let attempt = 0;
        let reachedConnected = false;

        dispatch({ type: 'reset' });

        const release = () => {
            signalling?.close();
            mesh?.close();
            peer?.close();
            signalling = null;
            peer = null;
            mesh = null;
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
                        onRemoteMedia: (streams) => {
                            if (mine === generation) dispatch({ type: 'remote_media_updated', streams });
                        },
                        onConnected: () => {
                            if (disposed || mine !== generation) return;
                            reachedConnected = true;
                            dispatch({ type: 'connected' });
                        },
                        onFailed: () => {
                            if (disposed || mine !== generation) return;
                            console.warn('peer connection failed, reconnecting');
                            scheduleReconnect(false);
                        },
                    });
                    peer = session;
                    mesh = startMesh({
                        selfId: message.payload.session_id,
                        iceServers: message.payload.ice_servers,
                        forceRelay,
                        localStream: local,
                        peer: () => peer,
                        signalling: () => signalling,
                        dispatch,
                        isCurrent: () => !disposed && mine === generation,
                    });
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
                case 'route':
                case 'link_close':
                case 'peer_offer':
                case 'peer_answer':
                case 'peer_ice':
                    await mesh?.handle(message);
                    break;
                case 'uplink':
                    await peer?.setUplinkActive(message.payload.active);
                    break;
                case 'peer_left':
                    mesh?.peerLeft(message.payload.session_id);
                    dispatch({ type: 'peer_left', sessionId: message.payload.session_id });
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
            dispatch({ type: 'socket_connecting' });
        }

        const start = async () => {
            try {
                const stream = await captureLocalMedia();
                if (disposed) {
                    stopMedia(stream);
                    return;
                }
                local = stream;
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
        };
    }, [channelId, forceRelay]);

    return state;
};
