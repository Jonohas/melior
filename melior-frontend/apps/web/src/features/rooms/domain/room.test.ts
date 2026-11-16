import { describe, expect, test } from 'vitest';
import { parseRoom, parseRooms, toRoomBody } from './room';

const wire = { channel_id: 'general', owner: 'alice', restricted: true, members: ['bob'], is_owner: false };

describe('parseRoom', () => {
    test('maps the wire shape to a Room', () => {
        expect(parseRoom(wire)).toEqual({
            channelId: 'general',
            owner: 'alice',
            restricted: true,
            members: ['bob'],
            isOwner: false,
        });
    });

    test('rejects malformed rooms', () => {
        expect(parseRoom(null)).toBeNull();
        expect(parseRoom([])).toBeNull();
        expect(parseRoom({ ...wire, channel_id: 1 })).toBeNull();
        expect(parseRoom({ ...wire, owner: undefined })).toBeNull();
        expect(parseRoom({ ...wire, restricted: 'yes' })).toBeNull();
        expect(parseRoom({ ...wire, is_owner: undefined })).toBeNull();
        expect(parseRoom({ ...wire, members: 'bob' })).toBeNull();
        expect(parseRoom({ ...wire, members: [1] })).toBeNull();
    });
});

describe('parseRooms', () => {
    test('parses a list and rejects it when any entry is bad', () => {
        expect(parseRooms([wire, { ...wire, channel_id: 'b' }])?.map((r) => r.channelId)).toEqual(['general', 'b']);
        expect(parseRooms([])).toEqual([]);
        expect(parseRooms([wire, {}])).toBeNull();
        expect(parseRooms({})).toBeNull();
    });
});

describe('toRoomBody', () => {
    test('sends only restricted and members', () => {
        expect(toRoomBody({ restricted: true, members: ['bob'] })).toEqual({ restricted: true, members: ['bob'] });
    });
});
