import { describe, expect, test } from 'vitest';
import { checkImageFile, describeImageRefusal, MAX_IMAGE_BYTES } from './image-file';

describe('checkImageFile', () => {
    test('accepts the four image types up to 10 MiB', () => {
        for (const type of ['image/png', 'image/jpeg', 'image/gif', 'image/webp']) {
            expect(checkImageFile({ type, size: 1 })).toEqual({ ok: true });
            expect(checkImageFile({ type, size: MAX_IMAGE_BYTES })).toEqual({ ok: true });
        }
        expect(MAX_IMAGE_BYTES).toBe(10 * 1024 * 1024);
    });

    test('refuses other types, empty files and files over 10 MiB', () => {
        for (const type of ['image/svg+xml', 'image/bmp', 'text/plain', 'application/pdf', '']) {
            expect(checkImageFile({ type, size: 10 })).toEqual({ ok: false, reason: 'unsupported_type' });
        }
        expect(checkImageFile({ type: 'image/png', size: 0 })).toEqual({ ok: false, reason: 'empty' });
        expect(checkImageFile({ type: 'image/png', size: MAX_IMAGE_BYTES + 1 })).toEqual({
            ok: false,
            reason: 'too_large',
        });
    });

    test('every refusal has words to show', () => {
        for (const reason of ['empty', 'unsupported_type', 'too_large'] as const) {
            expect(describeImageRefusal(reason)).not.toBe('');
        }
    });
});
