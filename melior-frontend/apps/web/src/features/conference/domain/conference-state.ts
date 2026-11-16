import type { ConferenceStatus } from './conference-status';
import type { StopReason } from './reconnect';
import type { SessionId } from './session-id';
import type { Participant } from './signalling-message';

/** Structural stand-in for MediaStream so the domain stays free of DOM types. */
export type StreamRef = object;

export type ConferenceState<TStream extends StreamRef> = {
    sessionId: SessionId | null;
    /** Own display name, as the server reports it. */
    username: string | null;
    /** Remote participants by session id (a remote stream's id is its publisher's session id). */
    participants: Record<SessionId, string>;
    localStream: TStream | null;
    /** One stream per remote participant that has media, keyed by session id. */
    remoteStreams: Record<SessionId, TStream>;
    status: ConferenceStatus;
    /** Consecutive reconnect attempts so far; 0 while a connection is healthy. */
    reconnectAttempt: number;
    /** Set when the conference gave up for good (`status` is `stopped`). */
    stopReason: StopReason | null;
    error: string | null;
};

export type ConferenceAction<TStream extends StreamRef> =
    | { type: 'reset' }
    | { type: 'local_stream_ready'; stream: TStream }
    | { type: 'socket_connecting' }
    | { type: 'session_started'; sessionId: SessionId; username: string; participants: Participant[] }
    | { type: 'peer_joined'; sessionId: SessionId; username: string }
    | { type: 'connected' }
    | { type: 'failed'; message: string }
    | { type: 'reconnecting'; attempt: number }
    | { type: 'stopped'; reason: StopReason }
    | { type: 'remote_media_updated'; streams: Record<SessionId, TStream> }
    | { type: 'peer_left'; sessionId: SessionId };

export const createInitialConferenceState = <TStream extends StreamRef>(): ConferenceState<TStream> => ({
    sessionId: null,
    username: null,
    participants: {},
    localStream: null,
    remoteStreams: {},
    status: 'starting',
    reconnectAttempt: 0,
    stopReason: null,
    error: null,
});

export const conferenceReducer = <TStream extends StreamRef>(
    state: ConferenceState<TStream>,
    action: ConferenceAction<TStream>,
): ConferenceState<TStream> => {
    switch (action.type) {
        case 'reset':
            return createInitialConferenceState<TStream>();
        case 'local_stream_ready':
            return { ...state, localStream: action.stream };
        case 'socket_connecting':
            return { ...state, status: 'connecting' };
        case 'session_started':
            return {
                ...state,
                sessionId: action.sessionId,
                username: action.username,
                participants: Object.fromEntries(
                    action.participants
                        .filter((p) => p.session_id !== action.sessionId)
                        .map((p) => [p.session_id, p.username]),
                ),
                status: 'negotiating',
            };
        case 'peer_joined':
            return action.sessionId === state.sessionId
                ? state
                : { ...state, participants: { ...state.participants, [action.sessionId]: action.username } };
        case 'connected':
            return { ...state, status: 'connected', reconnectAttempt: 0 };
        case 'failed':
            return { ...state, error: action.message, status: 'error' };
        case 'reconnecting':
            // The camera and microphone stay; everything the old connection owned is dropped.
            return {
                ...state,
                sessionId: null,
                participants: {},
                remoteStreams: {},
                status: 'reconnecting',
                reconnectAttempt: action.attempt,
                error: null,
            };
        case 'stopped':
            return {
                ...state,
                sessionId: null,
                participants: {},
                remoteStreams: {},
                status: 'stopped',
                stopReason: action.reason,
            };
        case 'remote_media_updated':
            return { ...state, remoteStreams: action.streams };
        case 'peer_left': {
            const { [action.sessionId]: _left, ...participants } = state.participants;
            const { [action.sessionId]: _media, ...remoteStreams } = state.remoteStreams;
            return { ...state, participants, remoteStreams };
        }
    }
};
