import { describe, expect, test } from 'vitest';
import {
    admitImages,
    checkAttachments,
    describeLeftOut,
    fitWithin,
    isAllowedGifUrl,
    MAX_GIF_URL_CHARS,
    MAX_IMAGES_PER_MESSAGE,
    type OutgoingAttachment,
    parseChatAttachment,
    parseChatAttachments,
    parseGif,
    parseUploadedImage,
} from './chat-attachment';

describe('isAllowedGifUrl', () => {
    test('accepts https URLs on the KLIPY media hosts', () => {
        for (const host of ['static.klipy.com', 'static1.klipy.com', 'static2.klipy.com']) {
            expect(isAllowedGifUrl(`https://${host}/ii/abc/1.gif`)).toBe(true);
        }
    });

    test('refuses other schemes, hosts, ports, credentials, junk and overlong URLs', () => {
        for (const url of [
            'http://static.klipy.com/a.gif',
            'https://api.klipy.com/a.gif',
            'https://static.klipy.com.evil.example/a.gif',
            'https://evil.example/static.klipy.com/a.gif',
            'https://static.klipy.com:8443/a.gif',
            'https://user@static.klipy.com/a.gif',
            'javascript:alert(1)',
            'not a url',
            '',
        ]) {
            expect(isAllowedGifUrl(url)).toBe(false);
        }
        const base = 'https://static.klipy.com/';
        expect(isAllowedGifUrl(base + 'a'.repeat(MAX_GIF_URL_CHARS - base.length))).toBe(true);
        expect(isAllowedGifUrl(base + 'a'.repeat(MAX_GIF_URL_CHARS - base.length + 1))).toBe(false);
    });
});

describe('parseGif', () => {
    const gif = { url: 'https://static.klipy.com/a.gif', width: 320, height: 240 };

    test('takes whole sides from 1 to 4096 and drops other fields', () => {
        expect(parseGif({ ...gif, extra: true })).toEqual({ kind: 'gif', ...gif });
        expect(parseGif({ ...gif, width: 1, height: 4096 })).toEqual({ kind: 'gif', ...gif, width: 1, height: 4096 });
    });

    test('refuses sides out of range or not whole', () => {
        for (const bad of [{ width: 0 }, { height: 4097 }, { width: 2.5 }, { width: -1 }, { height: Number.NaN }]) {
            expect(parseGif({ ...gif, ...bad })).toBeNull();
        }
        expect(parseGif(null)).toBeNull();
    });
});

describe('parseChatAttachment', () => {
    test('maps the two kinds and refuses any other', () => {
        expect(parseChatAttachment({ kind: 'image', id: 'u1', content_type: 'image/webp' })).toEqual({
            kind: 'image',
            id: 'u1',
            contentType: 'image/webp',
        });
        expect(parseChatAttachment({ kind: 'file', id: 'u1', content_type: 'text/plain' })).toBeNull();
        expect(parseChatAttachment({ id: 'u1', content_type: 'image/png' })).toBeNull();
    });
});

describe('parseUploadedImage', () => {
    test('maps the upload answer and refuses another shape', () => {
        expect(parseUploadedImage({ id: 'u1', content_type: 'image/png', size: 1234 })).toEqual({
            id: 'u1',
            contentType: 'image/png',
            size: 1234,
        });
        expect(parseUploadedImage({ id: '', content_type: 'image/png', size: 1 })).toBeNull();
        expect(parseUploadedImage({ id: 'u1', content_type: 'image/png', size: -1 })).toBeNull();
        expect(parseUploadedImage({ id: 'u1', size: 1 })).toBeNull();
        expect(parseUploadedImage('u1')).toBeNull();
    });
});

describe('fitWithin', () => {
    test('scales down to the box keeping the aspect ratio, and never scales up', () => {
        expect(fitWithin(640, 480)).toEqual({ width: 320, height: 240 });
        expect(fitWithin(480, 640)).toEqual({ width: 240, height: 320 });
        expect(fitWithin(100, 50)).toEqual({ width: 100, height: 50 });
        expect(fitWithin(4096, 1)).toEqual({ width: 320, height: 1 });
    });
});

const imagesOf = (n: number): OutgoingAttachment[] =>
    Array.from({ length: n }, (_, i) => ({ kind: 'image', id: `i${i}` }));
const gif: OutgoingAttachment = { kind: 'gif', url: 'https://static.klipy.com/a.gif', width: 10, height: 10 };

describe('parseChatAttachments', () => {
    test('absent is none, a list keeps its order, anything else or one bad item is null', () => {
        expect(parseChatAttachments(undefined)).toEqual([]);
        expect(parseChatAttachments([])).toEqual([]);
        const list = [
            { kind: 'image', id: 'b', content_type: 'image/png' },
            { kind: 'image', id: 'a', content_type: 'image/gif' },
        ];
        expect(parseChatAttachments(list)?.map((a) => (a.kind === 'image' ? a.id : ''))).toEqual(['b', 'a']);
        expect(parseChatAttachments(null)).toBeNull();
        expect(parseChatAttachments(list[0])).toBeNull();
        expect(parseChatAttachments([...list, { kind: 'pdf' }])).toBeNull();
    });
});

describe('checkAttachments', () => {
    test('accepts none, 1 to 10 images, or one GIF', () => {
        expect(checkAttachments([])).toEqual({ ok: true });
        expect(checkAttachments(imagesOf(1))).toEqual({ ok: true });
        expect(checkAttachments(imagesOf(MAX_IMAGES_PER_MESSAGE))).toEqual({ ok: true });
        expect(checkAttachments([gif])).toEqual({ ok: true });
    });

    test('refuses more than 10 images, a GIF with anything else, and an image twice', () => {
        expect(checkAttachments(imagesOf(MAX_IMAGES_PER_MESSAGE + 1))).toEqual({
            ok: false,
            reason: 'too_many_images',
        });
        expect(checkAttachments([gif, gif])).toEqual({ ok: false, reason: 'gif_not_alone' });
        expect(checkAttachments([...imagesOf(2), gif])).toEqual({ ok: false, reason: 'gif_not_alone' });
        expect(checkAttachments([...imagesOf(2), ...imagesOf(1)])).toEqual({ ok: false, reason: 'duplicate_image' });
    });
});

describe('admitImages', () => {
    test('takes the first picked ones up to the limit and counts the rest', () => {
        const picked = ['a', 'b', 'c', 'd'];
        expect(admitImages(0, picked)).toEqual({ admitted: picked, leftOut: 0 });
        expect(admitImages(8, picked)).toEqual({ admitted: ['a', 'b'], leftOut: 2 });
        expect(admitImages(MAX_IMAGES_PER_MESSAGE, picked)).toEqual({ admitted: [], leftOut: 4 });
        expect(
            admitImages(
                0,
                Array.from({ length: 12 }, (_, i) => i),
            ).admitted,
        ).toHaveLength(MAX_IMAGES_PER_MESSAGE);
        expect(admitImages(12, ['a'])).toEqual({ admitted: [], leftOut: 1 });
    });

    test('says how many were left out and why', () => {
        expect(describeLeftOut(1)).toBe('A message carries at most 10 images; 1 image was left out.');
        expect(describeLeftOut(3)).toBe('A message carries at most 10 images; 3 images were left out.');
    });
});
