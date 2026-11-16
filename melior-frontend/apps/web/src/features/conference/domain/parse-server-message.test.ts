import { describe, expect, test } from 'vitest';
import { parseServerMessage } from './parse-server-message';

const msg = (channel: string, payload: unknown) => JSON.stringify({ channel, payload });

describe('parseServerMessage', () => {
    test('parses a session message', () => {
        const parsed = parseServerMessage(
            msg('session', {
                session_id: 's1',
                channel_id: 'general',
                username: 'alice',
                participants: [{ session_id: 's0', username: 'bob' }],
                ice_servers: [{ urls: ['stun:x:1'] }, { urls: ['turn:y:2'], username: 'u', credential: 'c' }],
            }),
        );
        expect<unknown>(parsed).toEqual({
            channel: 'session',
            payload: {
                session_id: 's1',
                channel_id: 'general',
                username: 'alice',
                participants: [{ session_id: 's0', username: 'bob' }],
                ice_servers: [{ urls: ['stun:x:1'] }, { urls: ['turn:y:2'], username: 'u', credential: 'c' }],
            },
        });
    });

    test('accepts an empty or missing ice_servers list and a single url string', () => {
        const empty = parseServerMessage(
            msg('session', { session_id: 's', channel_id: 'c', username: 'u', ice_servers: [] }),
        );
        expect(empty?.channel === 'session' && empty.payload.ice_servers).toEqual([]);
        const missing = parseServerMessage(msg('session', { session_id: 's', channel_id: 'c', username: 'u' }));
        expect(missing?.channel === 'session' && missing.payload.ice_servers).toEqual([]);
        const single = parseServerMessage(
            msg('session', { session_id: 's', channel_id: 'c', username: 'u', ice_servers: [{ urls: 'stun:a' }] }),
        );
        expect(single?.channel === 'session' && single.payload.ice_servers).toEqual([{ urls: ['stun:a'] }]);
    });

    test('rejects a session without a username or with a malformed participant list', () => {
        const base = { session_id: 's', channel_id: 'c', ice_servers: [] };
        expect(parseServerMessage(msg('session', base))).toBeNull();
        expect(parseServerMessage(msg('session', { ...base, username: 7 }))).toBeNull();
        expect(parseServerMessage(msg('session', { ...base, username: 'u', participants: 'x' }))).toBeNull();
        expect(
            parseServerMessage(msg('session', { ...base, username: 'u', participants: [{ session_id: 'a' }] })),
        ).toBeNull();
    });

    test('parses closed with a known reason and rejects unknown ones', () => {
        for (const reason of ['session_ended', 'removed', 'room_moved']) {
            expect(parseServerMessage(msg('closed', { reason }))).toEqual({ channel: 'closed', payload: { reason } });
        }
        expect(parseServerMessage(msg('closed', { reason: 'bored' }))).toBeNull();
        expect(parseServerMessage(msg('closed', {}))).toBeNull();
        expect(parseServerMessage(msg('closed', null))).toBeNull();
    });

    test('parses peer_joined', () => {
        expect(parseServerMessage(msg('peer_joined', { session_id: 'x', username: 'bob' }))).toEqual({
            channel: 'peer_joined',
            payload: { session_id: 'x', username: 'bob' },
        });
        expect(parseServerMessage(msg('peer_joined', { session_id: 'x' }))).toBeNull();
    });

    test('rejects a session with a bad channel id or ice server', () => {
        expect(
            parseServerMessage(msg('session', { session_id: 's', channel_id: 'a/b', username: 'u', ice_servers: [] })),
        ).toBeNull();
        expect(
            parseServerMessage(
                msg('session', { session_id: 's', channel_id: 'c', username: 'u', ice_servers: [{ urls: 3 }] }),
            ),
        ).toBeNull();
        expect(parseServerMessage(msg('session', { channel_id: 'c', ice_servers: [] }))).toBeNull();
    });

    test('parses sdp answer and offer and checks the type', () => {
        expect(parseServerMessage(msg('sdp_answer', { type: 'answer', sdp: 'v=0' }))).toEqual({
            channel: 'sdp_answer',
            payload: { type: 'answer', sdp: 'v=0' },
        });
        expect(parseServerMessage(msg('sdp_offer', { type: 'offer', sdp: 'v=0' }))).toEqual({
            channel: 'sdp_offer',
            payload: { type: 'offer', sdp: 'v=0', tracks: [] },
        });
        expect(parseServerMessage(msg('sdp_offer', { type: 'answer', sdp: 'v=0' }))).toBeNull();
        expect(parseServerMessage(msg('sdp_answer', { type: 'answer' }))).toBeNull();
    });

    test('parses the media section assignments of an offer', () => {
        const tracks = [
            { mid: '2', session_id: 'a' },
            { mid: '3', session_id: 'a' },
        ];
        expect(parseServerMessage(msg('sdp_offer', { type: 'offer', sdp: 'v=0', tracks }))).toEqual({
            channel: 'sdp_offer',
            payload: { type: 'offer', sdp: 'v=0', tracks },
        });
        expect(parseServerMessage(msg('sdp_offer', { type: 'offer', sdp: 'v=0', tracks: [] }))).toMatchObject({
            payload: { tracks: [] },
        });
        expect(parseServerMessage(msg('sdp_offer', { type: 'offer', sdp: 'v=0', tracks: 'x' }))).toBeNull();
        expect(
            parseServerMessage(msg('sdp_offer', { type: 'offer', sdp: 'v=0', tracks: [{ mid: 2, session_id: 'a' }] })),
        ).toBeNull();
        expect(parseServerMessage(msg('sdp_offer', { type: 'offer', sdp: 'v=0', tracks: [{ mid: '2' }] }))).toBeNull();
    });

    test('parses ice candidates in camelCase', () => {
        const parsed = parseServerMessage(
            msg('ice', { candidate: 'candidate:1', sdpMid: '0', sdpMLineIndex: 0, usernameFragment: 'f' }),
        );
        expect<unknown>(parsed).toEqual({
            channel: 'ice',
            payload: { candidate: 'candidate:1', sdpMid: '0', sdpMLineIndex: 0, usernameFragment: 'f' },
        });
        expect(parseServerMessage(msg('ice', { candidate: 5 }))).toBeNull();
        expect(parseServerMessage(msg('ice', 'nope'))).toBeNull();
    });

    test('parses peer_left and error', () => {
        expect(parseServerMessage(msg('peer_left', { session_id: 'x' }))).toEqual({
            channel: 'peer_left',
            payload: { session_id: 'x' },
        });
        expect(parseServerMessage(msg('error', { message: 'boom' }))).toEqual({
            channel: 'error',
            payload: { message: 'boom' },
        });
        expect(parseServerMessage(msg('peer_left', {}))).toBeNull();
        expect(parseServerMessage(msg('error', {}))).toBeNull();
    });

    test('rejects malformed input', () => {
        expect(parseServerMessage('not json')).toBeNull();
        expect(parseServerMessage('null')).toBeNull();
        expect(parseServerMessage('[]')).toBeNull();
        expect(parseServerMessage('{}')).toBeNull();
        expect(parseServerMessage(JSON.stringify({ channel: 5, payload: {} }))).toBeNull();
        expect(parseServerMessage(msg('unknown', {}))).toBeNull();
    });
});
