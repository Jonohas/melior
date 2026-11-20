import { useEffect, useReducer, useRef } from 'react';
import { captureLocalMedia, createSignallingClient, stopMedia } from '@/features/conference/api';
import {
    type ChannelId,
    type ConferenceState,
    conferenceReducer,
    createInitialConferenceState,
    type RoomProbe,
    type ServerMessage,
    type SessionProbe,
} from '@/features/conference/domain';
import { createReconnector } from './reconnector';
import {
    type ConferenceConnection,
    handleServerMessage,
    type MessageContext,
    releaseConnection,
} from './session-messages';

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
        const connection: ConferenceConnection = { signalling: null, peer: null, mesh: null };
        let local: MediaStream | null = null;
        // Bumped on every teardown so late callbacks of an old connection are ignored.
        let generation = 0;

        dispatch({ type: 'reset' });

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
            checkRoomAccess: checkRoomAccessRef,
        });

        const contextFor = (mine: number): MessageContext => ({
            connection,
            dispatch,
            forceRelay,
            localStream: local,
            isCurrent: () => !disposed && mine === generation,
            onConnected: reconnector.markConnected,
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
                channelId,
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
            reconnector.dispose();
            teardown();
            if (local) stopMedia(local);
        };
    }, [channelId, forceRelay]);

    return state;
};
