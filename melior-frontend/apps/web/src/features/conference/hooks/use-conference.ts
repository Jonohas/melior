import { useCallback, useEffect, useReducer, useRef } from 'react';
import { createSignallingClient, stopMedia } from '@/features/conference/api';
import {
    type ChannelId,
    type ChannelProbe,
    type ConferenceState,
    conferenceReducer,
    createInitialConferenceState,
    type ServerMessage,
    type SessionProbe,
} from '@/features/conference/domain';
import { createReconnector } from './reconnector';
import {
    type ChatEvent,
    type ConferenceConnection,
    handleServerMessage,
    type MessageContext,
    releaseConnection,
} from './session-messages';
import { type CallMedia, useCallMedia } from './use-call-media';
import { type ScreenShare, useScreenShare } from './use-screen-share';
import { useSpeaking } from './use-speaking';
import { useVoicePublisher } from './use-voice-publisher';

type UseConferenceReturn = ConferenceState<MediaStream> &
    Pick<
        CallMedia,
        'activeDevices' | 'changeInput' | 'microphoneMuted' | 'cameraOff' | 'setMicrophoneMuted' | 'setCameraOff'
    > & {
        /** Says something to the room. Returns false when there is no open connection, so nothing was sent. */
        sendChat: (text: string, clientId: string) => boolean;
        screenShareSupported: ScreenShare['supported'];
        screenSharing: ScreenShare['sharing'];
        screenError: ScreenShare['error'];
        startScreenShare: ScreenShare['start'];
        stopScreenShare: ScreenShare['stop'];
        /** Whether the own microphone picks up speech right now (always false while muted). */
        speaking: boolean;
    };

type UseConferenceOptions = {
    /** Route all media through the TURN relay (diagnostics). */
    forceRelay?: boolean;
    /**
     * Re-checks the login session after a lost connection (the browser reports no status for a refused
     * upgrade). The caller runs the login gate on 401 and shows the no-access state on 403.
     */
    checkSession?: () => Promise<SessionProbe>;
    /** Asks whether the user may join this voice channel (404 unknown, 403 refused, 409 full). */
    checkChannelAccess?: () => Promise<ChannelProbe>;
    /** Called for every chat message the room receives and every refusal of this client's messages. */
    onChat?: (event: ChatEvent) => void;
};

/**
 * Orchestrates one conference: camera once, then a signalling socket plus peer connection per
 * connection, replaced automatically after an unexpected loss. The decisions (what a close means, how
 * long to wait) are pure functions in the domain; this hook only executes them. With no channel there is
 * no call: nothing is opened, and a change of channel ends the old call before the new one starts.
 */
export const useConference = (channelId: ChannelId | null, options: UseConferenceOptions = {}): UseConferenceReturn => {
    const { forceRelay = false, checkSession, checkChannelAccess, onChat } = options;
    const checkSessionRef = useRef(checkSession);
    const checkChannelAccessRef = useRef(checkChannelAccess);
    const onChatRef = useRef(onChat);
    // The open connection, for the use cases that act outside the connection's own effect.
    const connectionRef = useRef<ConferenceConnection>({ signalling: null, peer: null, mesh: null });
    const media = useCallMedia(connectionRef);
    const screen = useScreenShare(connectionRef);
    const { open: openMedia, adopt: adoptMedia, release: releaseMedia } = media;
    const { resume: resumeScreen, release: releaseScreen, track: screenTrack } = screen;
    useEffect(() => {
        checkSessionRef.current = checkSession;
        checkChannelAccessRef.current = checkChannelAccess;
        onChatRef.current = onChat;
    });
    const [state, dispatch] = useReducer(
        conferenceReducer<MediaStream>,
        undefined,
        createInitialConferenceState<MediaStream>,
    );
    const speaking = useSpeaking(state.localStream, media.activeDevices.audioinput, media.microphoneMuted);
    useVoicePublisher({
        connection: connectionRef,
        sessionId: state.sessionId,
        switches: {
            muted: media.microphoneMuted,
            camera: !media.cameraOff,
            screen: screen.sharing,
            speaking,
        },
    });

    useEffect(() => {
        let disposed = false;
        const connection = connectionRef.current;
        let local: MediaStream | null = null;
        // Bumped on every teardown so late callbacks of an old connection are ignored.
        let generation = 0;

        dispatch({ type: 'reset' });
        if (channelId === null) return;
        const room = channelId;

        const teardown = () => {
            generation += 1;
            releaseConnection(connection);
        };

        const fail = (message: string) => {
            if (disposed) return;
            console.error(message);
            dispatch({ type: 'failed', message });
        };

        const reconnector = createReconnector({
            dispatch,
            isDisposed: () => disposed,
            generation: () => generation,
            teardown,
            release: () => releaseConnection(connection),
            connect: () => connect(),
            checkSession: checkSessionRef,
            checkChannelAccess: checkChannelAccessRef,
        });

        const contextFor = (mine: number): MessageContext => ({
            connection,
            dispatch,
            forceRelay,
            localStream: local,
            screenTrack,
            isCurrent: () => !disposed && mine === generation,
            onConnected: () => {
                reconnector.markConnected();
                // A screen shared before a reconnect goes on being shared.
                if (connection.peer) resumeScreen(connection.peer);
            },
            onChat: (event) => onChatRef.current?.(event),
            onPeerFailed: () => reconnector.scheduleReconnect(false),
            // The socket is about to close; act now instead of waiting for the close event.
            onServerClosed: (reason) => reconnector.onClosed(reason, true),
            onServerError: (message) => {
                teardown();
                fail(`Server error: ${message}`);
            },
        });

        const handle = async (message: ServerMessage, mine: number) => {
            if (disposed || mine !== generation) return;
            await handleServerMessage(contextFor(mine), message);
        };

        function connect() {
            if (disposed) return;
            const mine = generation;
            connection.signalling = createSignallingClient({
                channelId: room,
                onMessage: (message) => handle(message, mine),
                onClose: ({ opened }) => {
                    if (disposed || mine !== generation) return;
                    reconnector.onClosed(null, opened);
                },
            });
            dispatch({ type: 'socket_connecting' });
        }

        const start = async () => {
            try {
                const stream = await openMedia();
                if (disposed) {
                    stopMedia(stream);
                    return;
                }
                local = stream;
                adoptMedia(stream);
                dispatch({ type: 'local_stream_ready', stream });
                connect();
            } catch (error) {
                fail(`Could not access the camera: ${error instanceof Error ? error.message : String(error)}`);
            }
        };

        void start();

        return () => {
            disposed = true;
            reconnector.dispose();
            teardown();
            releaseMedia();
            releaseScreen();
        };
    }, [channelId, forceRelay, openMedia, adoptMedia, releaseMedia, resumeScreen, releaseScreen, screenTrack]);

    const sendChat = useCallback(
        (text: string, clientId: string) =>
            connectionRef.current.signalling?.send({
                channel: 'chat_send',
                payload: { text, client_id: clientId },
            }) ?? false,
        [],
    );

    return {
        ...state,
        sendChat,
        speaking,
        activeDevices: media.activeDevices,
        changeInput: media.changeInput,
        microphoneMuted: media.microphoneMuted,
        cameraOff: media.cameraOff,
        setMicrophoneMuted: media.setMicrophoneMuted,
        setCameraOff: media.setCameraOff,
        screenShareSupported: screen.supported,
        screenSharing: screen.sharing,
        screenError: screen.error,
        startScreenShare: screen.start,
        stopScreenShare: screen.stop,
    };
};
