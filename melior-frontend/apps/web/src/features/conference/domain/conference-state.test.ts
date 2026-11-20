import { describe, expect, test } from 'vitest';
import { type ConferenceAction, conferenceReducer, createInitialConferenceState } from './conference-state';
import { streamKey } from './remote-tracks';

type Stream = { id: string };

const run = (...actions: ConferenceAction<Stream>[]) =>
    actions.reduce(conferenceReducer<Stream>, createInitialConferenceState<Stream>());

describe('conferenceReducer', () => {
    test('starts in the starting status with no streams', () => {
        expect(createInitialConferenceState<Stream>()).toEqual({
            sessionId: null,
            username: null,
            participants: {},
            localStream: null,
            remoteStreams: {},
            screenStreams: {},
            status: 'starting',
            reconnectAttempt: 0,
            stopReason: null,
            error: null,
        });
    });

    test('walks through connecting, negotiating and connected', () => {
        const state = run(
            { type: 'local_stream_ready', stream: { id: 'local' } },
            { type: 'socket_connecting' },
            { type: 'session_started', sessionId: 's1', username: 'alice', participants: [] },
        );
        expect(state.status).toBe('negotiating');
        expect(state.sessionId).toBe('s1');
        expect(state.localStream).toEqual({ id: 'local' });
        expect(conferenceReducer(state, { type: 'connected' }).status).toBe('connected');
    });

    test('remote media is replaced as a whole and keyed by participant', () => {
        const a = { id: 'x' };
        const b = { id: 'y' };
        const state = run({ type: 'remote_media_updated', streams: { s2: a, s3: b } });
        expect(state.remoteStreams).toEqual({ s2: a, s3: b });
        expect(state.remoteStreams.s2).toBe(a);
        expect(conferenceReducer(state, { type: 'remote_media_updated', streams: { s3: b } }).remoteStreams).toEqual({
            s3: b,
        });
    });

    test("peer_left drops the participant's media", () => {
        const base = run({ type: 'remote_media_updated', streams: { a: { id: '1' }, b: { id: '2' } } });
        expect(Object.keys(conferenceReducer(base, { type: 'peer_left', sessionId: 'a' }).remoteStreams)).toEqual([
            'b',
        ]);
        expect(Object.keys(conferenceReducer(base, { type: 'peer_left', sessionId: 'zzz' }).remoteStreams)).toEqual([
            'a',
            'b',
        ]);
    });

    test('seeds the roster from the session and ignores the own entry', () => {
        const state = run({
            type: 'session_started',
            sessionId: 's1',
            username: 'alice',
            participants: [
                { session_id: 's1', username: 'alice' },
                { session_id: 's2', username: 'bob' },
            ],
        });
        expect(state.username).toBe('alice');
        expect(state.participants).toEqual({ s2: 'bob' });
    });

    test('a shared screen is kept apart from the camera of the same participant', () => {
        const state = run({
            type: 'remote_media_updated',
            streams: { s2: { id: 'cam' }, [streamKey('s2', 'screen')]: { id: 'scr' } },
        });
        expect(state.remoteStreams).toEqual({ s2: { id: 'cam' } });
        expect(state.screenStreams).toEqual({ s2: { id: 'scr' } });
    });

    test('the screen goes away when sharing stops, and when the participant leaves', () => {
        const sharing = run(
            { type: 'peer_joined', sessionId: 's2', username: 'bob' },
            {
                type: 'remote_media_updated',
                streams: { s2: { id: 'cam' }, [streamKey('s2', 'screen')]: { id: 'scr' } },
            },
        );
        const stopped = conferenceReducer(sharing, { type: 'remote_media_updated', streams: { s2: { id: 'cam' } } });
        expect(stopped.screenStreams).toEqual({});
        expect(stopped.remoteStreams).toEqual({ s2: { id: 'cam' } });

        const left = conferenceReducer(sharing, { type: 'peer_left', sessionId: 's2' });
        expect(left.screenStreams).toEqual({});
        expect(left.remoteStreams).toEqual({});
    });

    test('peer_joined adds to the roster and peer_left removes the entry and its stream', () => {
        const state = run(
            { type: 'session_started', sessionId: 's1', username: 'alice', participants: [] },
            { type: 'peer_joined', sessionId: 's2', username: 'bob' },
            { type: 'peer_joined', sessionId: 's3', username: 'carol' },
            { type: 'peer_joined', sessionId: 's1', username: 'alice' },
            { type: 'remote_media_updated', streams: { s2: { id: 'm' } } },
            { type: 'peer_left', sessionId: 's2' },
        );
        expect(state.participants).toEqual({ s3: 'carol' });
        expect(state.remoteStreams).toEqual({});
    });

    test('failed sets error status and message', () => {
        const state = run({ type: 'failed', message: 'camera denied' });
        expect(state.status).toBe('error');
        expect(state.error).toBe('camera denied');
    });

    test('reconnecting drops the old connection but keeps the local stream', () => {
        const state = run(
            { type: 'local_stream_ready', stream: { id: 'local' } },
            {
                type: 'session_started',
                sessionId: 's1',
                username: 'alice',
                participants: [{ session_id: 's2', username: 'bob' }],
            },
            { type: 'remote_media_updated', streams: { s2: { id: 'm' } } },
            { type: 'connected' },
            { type: 'reconnecting', attempt: 2 },
        );
        expect(state.status).toBe('reconnecting');
        expect(state.reconnectAttempt).toBe(2);
        expect(state.localStream).toEqual({ id: 'local' });
        expect(state.remoteStreams).toEqual({});
        expect(state.participants).toEqual({});
        expect(state.sessionId).toBeNull();
        expect(state.error).toBeNull();
    });

    test('connected resets the attempt counter', () => {
        expect(run({ type: 'reconnecting', attempt: 4 }, { type: 'connected' }).reconnectAttempt).toBe(0);
    });

    test('stopped records why and clears the room', () => {
        const state = run(
            { type: 'remote_media_updated', streams: { a: { id: 'm' } } },
            { type: 'stopped', reason: 'removed' },
        );
        expect(state.status).toBe('stopped');
        expect(state.stopReason).toBe('removed');
        expect(state.remoteStreams).toEqual({});
    });

    test('reset returns to the initial state', () => {
        const state = run(
            { type: 'remote_media_updated', streams: { a: { id: 'm' } } },
            { type: 'failed', message: 'x' },
            { type: 'reset' },
        );
        expect(state).toEqual(createInitialConferenceState<Stream>());
    });
});
