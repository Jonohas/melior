import { describe, expect, test } from 'vitest';
import { E2EE_VERSION, encodeKeyMessage, fromBase64Url, pairSalt, parseKeyMessage, toBase64Url } from './e2ee-envelope';
import {
    clearPrefixLength,
    FRAME_MAGIC,
    frameNonce,
    readTrailer,
    TAG_BYTES,
    TRAILER_BYTES,
    writeTrailer,
} from './e2ee-frame';
import { KEYS_KEPT_PER_SENDER, keepNewestKeys, mayReceiveKeys, summarizeEncryption } from './e2ee-trust';

const bytes = (length: number, fill = 7) => new Uint8Array(length).fill(fill);
const encodeRaw = (value: unknown) => toBase64Url(new TextEncoder().encode(JSON.stringify(value)));

describe('e2ee frame layout', () => {
    test('keeps the VP8 header and the Opus TOC byte in the clear, never more than the frame has', () => {
        expect(clearPrefixLength('video', 'key', 500)).toBe(10);
        expect(clearPrefixLength('video', 'delta', 500)).toBe(3);
        expect(clearPrefixLength('audio', undefined, 80)).toBe(1);
        expect(clearPrefixLength('video', 'key', 4)).toBe(4);
    });

    test('a trailer reads back the key id and counter it was written with', () => {
        const frame = new Uint8Array([
            ...bytes(3),
            ...bytes(20),
            ...bytes(TAG_BYTES),
            ...writeTrailer(0xdeadbeef, 2 ** 40),
        ]);
        expect(readTrailer(frame, 3)).toEqual({ keyId: 0xdeadbeef, counter: 2 ** 40 });
        expect(frame.at(-1)).toBe(FRAME_MAGIC);
    });

    test('a frame without the magic byte, or too short for a tag and trailer, is not encrypted', () => {
        expect(readTrailer(bytes(100, 0), 3)).toBeNull();
        expect(readTrailer(new Uint8Array([...bytes(3), ...writeTrailer(1, 1)]), 3)).toBeNull();
        expect(TRAILER_BYTES).toBe(13);
    });

    test('nonces differ by key id and by counter', () => {
        const seen = new Set([frameNonce(1, 0), frameNonce(1, 1), frameNonce(2, 0)].map((n) => n.join(',')));
        expect(seen.size).toBe(3);
        expect(frameNonce(1, 0)).toHaveLength(12);
    });
});

describe('e2ee key messages', () => {
    const hello = {
        type: 'hello' as const,
        device: bytes(65, 1),
        ephemeral: bytes(65, 2),
        signature: bytes(64, 3),
        certificate: 'a.b.c',
    };
    const key = { type: 'key' as const, keyId: 42, iv: bytes(12, 4), wrapped: bytes(32, 5) };

    test('a hello and a wrapped key survive the round trip', () => {
        expect(parseKeyMessage(encodeKeyMessage(hello))).toEqual(hello);
        expect(parseKeyMessage(encodeKeyMessage({ ...hello, certificate: null }))).toEqual({
            ...hello,
            certificate: null,
        });
        expect(parseKeyMessage(encodeKeyMessage(key))).toEqual(key);
    });

    test('rejects malformed, oversized, foreign-version and unknown messages', () => {
        const good = { v: E2EE_VERSION, t: 'key', k: 1, i: toBase64Url(bytes(12)), w: toBase64Url(bytes(32)) };
        for (const bad of [
            'not base64!',
            toBase64Url(new TextEncoder().encode('not json')),
            encodeRaw([1, 2]),
            encodeRaw({ ...good, v: 2 }),
            encodeRaw({ ...good, t: 'other' }),
            encodeRaw({ ...good, k: -1 }),
            encodeRaw({ ...good, k: 2 ** 32 }),
            encodeRaw({ ...good, k: 1.5 }),
            encodeRaw({ ...good, i: toBase64Url(bytes(11)) }),
            encodeRaw({ ...good, w: 'x' }),
            encodeRaw({ v: 1, t: 'hello', d: toBase64Url(bytes(65)), e: toBase64Url(bytes(65)), s: 5 }),
            encodeRaw({
                v: 1,
                t: 'hello',
                d: toBase64Url(bytes(65)),
                e: toBase64Url(bytes(65)),
                s: toBase64Url(bytes(64)),
                c: 7,
            }),
        ]) {
            expect(parseKeyMessage(bad)).toBeNull();
        }
        expect(parseKeyMessage(encodeRaw(good))).not.toBeNull();
    });

    test('base64url round-trips every byte value and refuses padding and foreign characters', () => {
        const all = Uint8Array.from({ length: 256 }, (_, i) => i);
        expect(fromBase64Url(toBase64Url(all))).toEqual(all);
        expect(fromBase64Url('ab==')).toBeNull();
        expect(fromBase64Url('a+b')).toBeNull();
    });

    test('both sides of a pair derive the same salt', () => {
        expect(pairSalt('b', 'a')).toEqual(pairSalt('a', 'b'));
    });
});

describe('device trust', () => {
    test('a verified device receives keys; an unverified one only when no realm vouches', () => {
        expect(mayReceiveKeys('verified', true)).toBe(true);
        expect(mayReceiveKeys('unverified', true)).toBe(false);
        expect(mayReceiveKeys('unverified', false)).toBe(true);
        expect(mayReceiveKeys('invalid', false)).toBe(false);
        expect(mayReceiveKeys('pending', false)).toBe(false);
    });
});

describe('key ring', () => {
    test('keeps the newest keys of a sender and forgets the rest', () => {
        let held: number[] = [];
        const forgotten: number[] = [];
        for (const keyId of [1, 2, 3, 4, 5]) {
            const next = keepNewestKeys(held, keyId);
            held = next.keep;
            forgotten.push(...next.forget);
        }
        expect(held).toEqual([3, 4, 5]);
        expect(held).toHaveLength(KEYS_KEPT_PER_SENDER);
        expect(forgotten).toEqual([1, 2]);
    });

    test('a key that arrives again moves to the newest place instead of being held twice', () => {
        expect(keepNewestKeys([1, 2], 1)).toEqual({ keep: [2, 1], forget: [] });
    });
});

describe('encryption summary', () => {
    const people = { a: 'alice', b: 'bob' };

    test('alone, all verified, or still waiting for keys', () => {
        expect(summarizeEncryption({}, {})).toEqual({ kind: 'alone' });
        expect(summarizeEncryption(people, { a: 'verified', b: 'verified' })).toEqual({ kind: 'secure' });
        expect(summarizeEncryption(people, { a: 'verified' })).toEqual({ kind: 'waiting', peers: ['bob'] });
    });

    test('an unverified device or a failed hello needs attention, and outranks waiting', () => {
        expect(summarizeEncryption(people, { a: 'unverified', b: 'pending' })).toEqual({
            kind: 'attention',
            unverified: ['alice'],
            invalid: [],
            self: false,
        });
        expect(summarizeEncryption(people, { a: 'invalid', b: 'verified' })).toEqual({
            kind: 'attention',
            unverified: [],
            invalid: ['alice'],
            self: false,
        });
    });

    test('an own device the realm did not vouch for needs attention even when everyone else is verified', () => {
        expect(summarizeEncryption(people, { a: 'verified', b: 'verified' }, 'unverified')).toEqual({
            kind: 'attention',
            unverified: [],
            invalid: [],
            self: true,
        });
    });
});
