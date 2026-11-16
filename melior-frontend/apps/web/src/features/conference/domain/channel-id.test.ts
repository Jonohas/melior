import { describe, expect, test } from 'vitest';
import { DEFAULT_CHANNEL_ID, isChannelId, normalizeChannelInput, parseChannelId } from './channel-id';

describe('isChannelId', () => {
    test('accepts letters, digits, underscore and dash', () => {
        expect(isChannelId('my-room_1')).toBe(true);
        expect(isChannelId('a'.repeat(64))).toBe(true);
    });

    test('rejects empty, too long, and illegal characters', () => {
        expect(isChannelId('')).toBe(false);
        expect(isChannelId('a'.repeat(65))).toBe(false);
        expect(isChannelId('a/b')).toBe(false);
        expect(isChannelId('a b')).toBe(false);
        expect(isChannelId('a\n')).toBe(false);
        expect(isChannelId(42)).toBe(false);
    });
});

describe('parseChannelId', () => {
    test('returns valid values unchanged', () => {
        expect<string>(parseChannelId('room')).toBe('room');
    });

    test('falls back to the default channel', () => {
        expect<string>(parseChannelId(null)).toBe(DEFAULT_CHANNEL_ID);
        expect<string>(parseChannelId(undefined)).toBe(DEFAULT_CHANNEL_ID);
        expect<string>(parseChannelId('bad/room')).toBe(DEFAULT_CHANNEL_ID);
        expect<string>(DEFAULT_CHANNEL_ID).toBe('general');
    });
});

describe('normalizeChannelInput', () => {
    test('trims, defaults empty input and rejects invalid names', () => {
        expect<string | null>(normalizeChannelInput('  room-1 ')).toBe('room-1');
        expect<string | null>(normalizeChannelInput('')).toBe('general');
        expect<string | null>(normalizeChannelInput('   ')).toBe('general');
        expect(normalizeChannelInput('bad name')).toBeNull();
        expect(normalizeChannelInput('a/b')).toBeNull();
    });
});
