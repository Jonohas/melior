import type { ConferenceStatus } from './conference-status';
import type { DeviceTrust } from './e2ee-trust';
import type { StopReason } from './reconnect';
import { partitionStreams, type StreamKey } from './remote-tracks';
import type { SessionId } from './session-id';
import type { Participant, Path } from './signalling-message';
import type { VoiceState } from './voice-state';

/** Structural stand-in for MediaStream so the domain stays free of DOM types. */
export type StreamRef = object;

export type ConferenceState<TStream extends StreamRef> = {
    sessionId: SessionId | null;
    /** Own display name, as the server reports it. */
    username: string | null;
    /** Remote participants by session id (a remote stream's id is its publisher's session id). */
    participants: Record<SessionId, string>;
    /** Mute, camera, screen and speaking per participant (this one included), as the server relays them. */
    voices: Record<SessionId, VoiceState>;
    localStream: TStream | null;
    /** One stream per remote participant that has media, keyed by session id. */
    remoteStreams: Record<SessionId, TStream>;
    /** The direct-link stream per remote participant, while a direct link delivers media. */
    directStreams: Record<SessionId, TStream>;
    /** The screen each remote participant shares over its direct link, while it says it shares one. */
    directScreens: Record<SessionId, TStream>;
    /** The path each remote participant is currently shown over; absent means `forwarded`. */
    shownPaths: Record<SessionId, Path>;
    /** Screens that participants share, by the session id of the one sharing. */
    screenStreams: Record<SessionId, TStream>;
    /** How far each remote participant's device is trusted for end-to-end encryption. */
    trust: Record<SessionId, DeviceTrust>;
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
    | { type: 'peer_joined'; sessionId: SessionId; username: string; voice: VoiceState }
    | { type: 'voice_changed'; sessionId: SessionId; voice: VoiceState }
    | { type: 'connected' }
    | { type: 'failed'; message: string }
    | { type: 'reconnecting'; attempt: number }
    | { type: 'stopped'; reason: StopReason }
    | { type: 'remote_media_updated'; streams: Record<StreamKey, TStream> }
    | { type: 'direct_media_updated'; sessionId: SessionId; stream: TStream | null }
    | { type: 'direct_screen_updated'; sessionId: SessionId; stream: TStream | null }
    | { type: 'shown_path_changed'; sessionId: SessionId; path: Path }
    | { type: 'trust_changed'; sessionId: SessionId; trust: DeviceTrust }
    | { type: 'peer_left'; sessionId: SessionId };

export const createInitialConferenceState = <TStream extends StreamRef>(): ConferenceState<TStream> => ({
    sessionId: null,
    username: null,
    participants: {},
    voices: {},
    localStream: null,
    remoteStreams: {},
    directStreams: {},
    directScreens: {},
    shownPaths: {},
    screenStreams: {},
    trust: {},
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
                voices: Object.fromEntries(action.participants.map((p) => [p.session_id, p.voice])),
                status: 'negotiating',
            };
        case 'peer_joined':
            return action.sessionId === state.sessionId
                ? state
                : {
                      ...state,
                      participants: { ...state.participants, [action.sessionId]: action.username },
                      voices: { ...state.voices, [action.sessionId]: action.voice },
                  };
        case 'voice_changed':
            return { ...state, voices: { ...state.voices, [action.sessionId]: action.voice } };
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
                voices: {},
                remoteStreams: {},
                directStreams: {},
                directScreens: {},
                shownPaths: {},
                screenStreams: {},
                trust: {},
                status: 'reconnecting',
                reconnectAttempt: action.attempt,
                error: null,
            };
        case 'stopped':
            return {
                ...state,
                sessionId: null,
                participants: {},
                voices: {},
                remoteStreams: {},
                directStreams: {},
                directScreens: {},
                shownPaths: {},
                screenStreams: {},
                trust: {},
                status: 'stopped',
                stopReason: action.reason,
            };
        case 'remote_media_updated': {
            const { camera, screen } = partitionStreams(action.streams);
            return { ...state, remoteStreams: camera, screenStreams: screen };
        }
        case 'direct_media_updated': {
            const { [action.sessionId]: _old, ...others } = state.directStreams;
            return {
                ...state,
                directStreams: action.stream ? { ...others, [action.sessionId]: action.stream } : others,
            };
        }
        case 'direct_screen_updated': {
            const { [action.sessionId]: _old, ...others } = state.directScreens;
            return {
                ...state,
                directScreens: action.stream ? { ...others, [action.sessionId]: action.stream } : others,
            };
        }
        case 'trust_changed':
            return { ...state, trust: { ...state.trust, [action.sessionId]: action.trust } };
        case 'shown_path_changed':
            return { ...state, shownPaths: { ...state.shownPaths, [action.sessionId]: action.path } };
        case 'peer_left': {
            const { [action.sessionId]: _left, ...participants } = state.participants;
            const { [action.sessionId]: _voice, ...voices } = state.voices;
            const { [action.sessionId]: _media, ...remoteStreams } = state.remoteStreams;
            const { [action.sessionId]: _screen, ...screenStreams } = state.screenStreams;
            const { [action.sessionId]: _direct, ...directStreams } = state.directStreams;
            const { [action.sessionId]: _directScreen, ...directScreens } = state.directScreens;
            const { [action.sessionId]: _shown, ...shownPaths } = state.shownPaths;
            const { [action.sessionId]: _trust, ...trust } = state.trust;
            return {
                ...state,
                participants,
                voices,
                remoteStreams,
                screenStreams,
                directStreams,
                directScreens,
                shownPaths,
                trust,
            };
        }
    }
};
