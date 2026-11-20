import { describe, expect, test } from 'vitest';
import { isChannelId } from './channel-id';

describe('isChannelId', () => {
    test('accepts letters, digits, underscore and dash', () => {
        expect(isChannelId('my-room_1')).toBe(true);
        expect(isChannelId('a'.repeat(64))).toBe(true);
        expect(isChannelId('4b1f3c2e-8d7a-4e6b-9c1d-2f3a4b5c6d7e')).toBe(true);
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
