import { describe, expect, test } from 'vitest';
import { buildRed, parseRed } from './red-payload';

const bytes = (...values: number[]) => new Uint8Array(values);

describe('RED payloads', () => {
    test('a payload with one redundant block splits into its blocks and builds back the same bytes', () => {
        // F=1 PT=111, offset 960, length 3; then F=0 PT=111; then 3 redundant bytes and 2 primary bytes.
        const payload = bytes(0xef, 0x0f, 0x00, 0x03, 0x6f, 1, 2, 3, 9, 8);
        const red = parseRed(payload);
        expect(red?.redundant).toEqual([{ payloadType: 111, timestampOffset: 960, data: bytes(1, 2, 3) }]);
        expect(red?.primary).toEqual({ payloadType: 111, timestampOffset: 0, data: bytes(9, 8) });
        expect(red && buildRed(red)).toEqual(payload);
    });

    test('a payload with only a primary block has a one-byte header', () => {
        expect(parseRed(bytes(0x6f, 5, 6))).toEqual({
            redundant: [],
            primary: { payloadType: 111, timestampOffset: 0, data: bytes(5, 6) },
        });
    });

    test('lengths are rewritten when blocks change size', () => {
        const red = {
            redundant: [{ payloadType: 111, timestampOffset: 960, data: new Uint8Array(300) }],
            primary: { payloadType: 111, timestampOffset: 0, data: new Uint8Array(40) },
        };
        const parsed = parseRed(buildRed(red));
        expect(parsed?.redundant[0]?.data.length).toBe(300);
        expect(parsed?.primary.data.length).toBe(40);
    });

    test('a redundant block too long for its header is left out, the primary kept', () => {
        const red = {
            redundant: [{ payloadType: 111, timestampOffset: 960, data: new Uint8Array(1024) }],
            primary: { payloadType: 111, timestampOffset: 0, data: bytes(7) },
        };
        expect(parseRed(buildRed(red))).toEqual({
            redundant: [],
            primary: { payloadType: 111, timestampOffset: 0, data: bytes(7) },
        });
    });

    test('a malformed payload is refused', () => {
        expect(parseRed(bytes())).toBeNull();
        expect(parseRed(bytes(0xef, 0x0f))).toBeNull();
        expect(parseRed(bytes(0xef, 0x0f, 0x00, 0x09, 0x6f, 1))).toBeNull();
    });
});
