import { describe, expect, test } from 'vitest';
import { buildRed, parseRed } from '@/features/conference/domain';
import { createMediaKey } from './e2ee-crypto';
import { decryptFrame, decryptRedFrame, encryptFrame, encryptRedFrame } from './frame-cipher';

const frameOf = (...values: number[]) => new Uint8Array(values).buffer;
const vp8Keyframe = () => frameOf(0x10, 0x02, 0x00, 0x9d, 0x01, 0x2a, 0x80, 0x02, 0xe0, 0x01, ...Array(50).fill(0xab));

describe('frame cipher', () => {
    test('a frame decrypts to what was encrypted, with the codec header left in the clear', async () => {
        const media = await createMediaKey();
        const sending = { keyId: media.keyId, key: media.key, counter: 0 };
        const plain = vp8Keyframe();
        const sealed = await encryptFrame(plain, 'video', 'key', sending);
        expect(new Uint8Array(sealed).subarray(0, 10)).toEqual(new Uint8Array(plain).subarray(0, 10));
        expect(new Uint8Array(sealed).subarray(10, 60)).not.toEqual(new Uint8Array(plain).subarray(10, 60));
        expect(sending.counter).toBe(1);
        const opened = await decryptFrame(sealed, 'video', 'key', (id) => (id === media.keyId ? [media.key] : []));
        expect(opened && new Uint8Array(opened)).toEqual(new Uint8Array(plain));
    });

    test('audio keeps its TOC byte in the clear', async () => {
        const media = await createMediaKey();
        const sealed = await encryptFrame(frameOf(0x78, 1, 2, 3), 'audio', undefined, {
            ...media,
            counter: 5,
        });
        expect(new Uint8Array(sealed)[0]).toBe(0x78);
        const opened = await decryptFrame(sealed, 'audio', undefined, () => [media.key]);
        expect(opened && [...new Uint8Array(opened)]).toEqual([0x78, 1, 2, 3]);
    });

    test('an unknown key, a wrong key, a changed clear byte or an unencrypted frame is dropped', async () => {
        const media = await createMediaKey();
        const other = await createMediaKey();
        const sealed = await encryptFrame(vp8Keyframe(), 'video', 'key', { ...media, counter: 0 });
        expect(await decryptFrame(sealed, 'video', 'key', () => [])).toBeNull();
        expect(await decryptFrame(sealed, 'video', 'key', () => [other.key])).toBeNull();
        const tampered = new Uint8Array(sealed.slice(0));
        tampered[4] = 0x00;
        expect(await decryptFrame(tampered.buffer, 'video', 'key', () => [media.key])).toBeNull();
        expect(await decryptFrame(vp8Keyframe(), 'video', 'key', () => [media.key])).toBeNull();
    });

    test('with clashing key ids, the key that opens the frame wins', async () => {
        const media = await createMediaKey();
        const other = await createMediaKey();
        const sealed = await encryptFrame(vp8Keyframe(), 'video', 'key', { ...media, counter: 0 });
        expect(await decryptFrame(sealed, 'video', 'key', () => [other.key, media.key])).not.toBeNull();
    });

    test('a RED frame keeps its headers readable, and each block opens on its own', async () => {
        const media = await createMediaKey();
        const keys = () => [media.key];
        const plain = buildRed({
            redundant: [{ payloadType: 111, timestampOffset: 960, data: new Uint8Array([0x78, 1, 2, 3]) }],
            primary: { payloadType: 111, timestampOffset: 0, data: new Uint8Array([0x78, 4, 5, 6, 7]) },
        });
        const sealed = await encryptRedFrame(plain.buffer, { ...media, counter: 0 });
        const red = parseRed(new Uint8Array(sealed));
        expect(red?.redundant).toHaveLength(1);
        expect(red?.primary.payloadType).toBe(111);
        // What the SFU sends a participant without RED: the primary block alone, an ordinary encrypted Opus frame.
        const primaryOnly = await decryptFrame(
            red?.primary.data.slice().buffer ?? new ArrayBuffer(0),
            'audio',
            undefined,
            keys,
        );
        expect(primaryOnly && [...new Uint8Array(primaryOnly)]).toEqual([0x78, 4, 5, 6, 7]);
        const opened = await decryptRedFrame(sealed, keys);
        expect(opened && new Uint8Array(opened)).toEqual(plain);
    });

    test('a RED frame whose primary block does not open is dropped', async () => {
        const media = await createMediaKey();
        const other = await createMediaKey();
        const plain = buildRed({
            redundant: [],
            primary: { payloadType: 111, timestampOffset: 0, data: new Uint8Array([0x78, 1]) },
        });
        const sealed = await encryptRedFrame(plain.buffer, { ...media, counter: 0 });
        expect(await decryptRedFrame(sealed, () => [other.key])).toBeNull();
    });
});
