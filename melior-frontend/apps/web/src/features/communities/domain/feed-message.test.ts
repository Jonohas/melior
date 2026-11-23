import { describe, expect, test } from 'vitest';
import { applyToRoster, decideFeedClose, describeRemoved, parseFeedMessage, removalAfterProbe } from './feed-message';
import { EMPTY_ROSTER, occupantsOf } from './voice-roster';

const msg = (channel: string, payload: unknown) => JSON.stringify({ channel, payload });
const wireEntry = {
    channel_id: 'v1',
    session_id: 's1',
    user_id: 'u1',
    username: 'bob',
    muted: false,
    camera: true,
    screen: false,
    speaking: true,
};
const entry = {
    channelId: 'v1',
    sessionId: 's1',
    userId: 'u1',
    username: 'bob',
    muted: false,
    camera: true,
    screen: false,
    speaking: true,
};

describe('parseFeedMessage', () => {
    test('parses the snapshot and voice changes', () => {
        expect(parseFeedMessage(msg('snapshot', { voice: [wireEntry] }))).toEqual({
            channel: 'snapshot',
            voice: [entry],
        });
        expect(parseFeedMessage(msg('voice_joined', wireEntry))).toEqual({ channel: 'voice_joined', entry });
        expect(parseFeedMessage(msg('voice_updated', wireEntry))).toEqual({ channel: 'voice_updated', entry });
        expect(parseFeedMessage(msg('voice_left', { channel_id: 'v1', session_id: 's1' }))).toEqual({
            channel: 'voice_left',
            channelId: 'v1',
            sessionId: 's1',
        });
    });

    test('parses messages, structure changes and removals', () => {
        const message = { id: 3, username: 'bob', text: 'hi', sent_at_ms: 10 };
        expect(parseFeedMessage(msg('message', { channel_id: 't1', message }))).toEqual({
            channel: 'message',
            channelId: 't1',
            message: { id: 3, username: 'bob', text: 'hi', sentAtMs: 10, attachment: null },
        });
        const attachment = { kind: 'image', id: 'a1', content_type: 'image/jpeg' };
        expect(
            parseFeedMessage(msg('message', { channel_id: 't1', message: { ...message, text: '', attachment } })),
        ).toMatchObject({ message: { text: '', attachment: { kind: 'image', id: 'a1', contentType: 'image/jpeg' } } });
        expect(parseFeedMessage(msg('structure_changed', {}))).toEqual({ channel: 'structure_changed' });
        expect(parseFeedMessage(msg('members_changed', {}))).toEqual({ channel: 'members_changed' });
        for (const reason of ['kicked', 'left', 'deleted']) {
            expect(parseFeedMessage(msg('removed', { reason }))).toEqual({ channel: 'removed', reason });
        }
        expect(parseFeedMessage(msg('closed', { reason: 'session_ended' }))).toEqual({
            channel: 'closed',
            reason: 'session_ended',
        });
        expect(parseFeedMessage(msg('closed', { reason: 'nap' }))).toBeNull();
    });

    test('rejects malformed and unknown frames', () => {
        expect(parseFeedMessage('nope')).toBeNull();
        expect(parseFeedMessage(msg('snapshot', { voice: [{ ...wireEntry, muted: 'x' }] }))).toBeNull();
        expect(parseFeedMessage(msg('snapshot', {}))).toBeNull();
        expect(parseFeedMessage(msg('voice_joined', { ...wireEntry, username: undefined }))).toBeNull();
        expect(parseFeedMessage(msg('voice_left', { channel_id: 'v1' }))).toBeNull();
        expect(parseFeedMessage(msg('message', { channel_id: 't1', message: { id: 'x' } }))).toBeNull();
        const gifFromElsewhere = { kind: 'gif', url: 'https://example.com/a.gif', width: 1, height: 1 };
        expect(
            parseFeedMessage(
                msg('message', {
                    channel_id: 't1',
                    message: { id: 3, username: 'bob', text: '', sent_at_ms: 10, attachment: gifFromElsewhere },
                }),
            ),
        ).toBeNull();
        expect(parseFeedMessage(msg('removed', { reason: 'bored' }))).toBeNull();
        expect(parseFeedMessage(msg('hello', {}))).toBeNull();
    });
});

describe('applyToRoster', () => {
    test('a snapshot replaces, joins and updates upsert, leaves remove', () => {
        const joined = applyToRoster(EMPTY_ROSTER, { channel: 'voice_joined', entry });
        expect(occupantsOf(joined, 'v1')).toEqual([entry]);
        const moved = applyToRoster(joined, { channel: 'voice_updated', entry: { ...entry, channelId: 'v2' } });
        expect(occupantsOf(moved, 'v1')).toEqual([]);
        expect(occupantsOf(moved, 'v2')).toHaveLength(1);
        const left = applyToRoster(moved, { channel: 'voice_left', channelId: 'v2', sessionId: 's1' });
        expect(left).toEqual({});
        const snapshot = applyToRoster(left, { channel: 'snapshot', voice: [entry, { ...entry, sessionId: 's2' }] });
        expect(occupantsOf(snapshot, 'v1').map((e) => e.sessionId)).toEqual(['s1', 's2']);
        expect(applyToRoster(snapshot, { channel: 'structure_changed' })).toBe(snapshot);
        expect(applyToRoster(snapshot, { channel: 'members_changed' })).toBe(snapshot);
        expect(applyToRoster(snapshot, { channel: 'removed', reason: 'kicked' })).toEqual({});
    });

    test('occupants are sorted by name', () => {
        const roster = applyToRoster(EMPTY_ROSTER, {
            channel: 'snapshot',
            voice: [
                { ...entry, sessionId: 'a', username: 'zed' },
                { ...entry, sessionId: 'b', username: 'amy' },
            ],
        });
        expect(occupantsOf(roster, 'v1').map((e) => e.username)).toEqual(['amy', 'zed']);
    });
});

describe('decideFeedClose', () => {
    test('stops after removed, checks first when it never opened or the session ended, else reconnects', () => {
        expect(decideFeedClose({ removed: true, sessionEnded: false, opened: true })).toBe('stop');
        expect(decideFeedClose({ removed: false, sessionEnded: false, opened: false })).toBe('check_membership');
        expect(decideFeedClose({ removed: false, sessionEnded: true, opened: true })).toBe('check_membership');
        expect(decideFeedClose({ removed: false, sessionEnded: false, opened: true })).toBe('reconnect');
        expect(describeRemoved('deleted')).toBe('The server was deleted.');
    });
});

describe('removalAfterProbe', () => {
    test('403 is kicked, 404 deleted, anything else keeps going', () => {
        expect(removalAfterProbe('forbidden')).toBe('kicked');
        expect(removalAfterProbe('not_found')).toBe('deleted');
        expect(removalAfterProbe('unavailable')).toBeNull();
        expect(removalAfterProbe(null)).toBeNull();
    });
});
