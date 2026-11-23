import { describe, expect, test } from 'vitest';
import { MAX_IMAGES_PER_MESSAGE } from './chat-attachment';
import { checkDraft, MAX_CHAT_CHARS } from './chat-text';

describe('checkDraft', () => {
    test('trims the text', () => {
        expect(checkDraft('  hello \n')).toEqual({ ok: true, text: 'hello' });
    });

    test('refuses an empty or blank draft', () => {
        expect(checkDraft('')).toEqual({ ok: false, reason: 'empty' });
        expect(checkDraft(' \n\t ')).toEqual({ ok: false, reason: 'empty' });
    });

    test('allows empty text only with an attachment, and still checks text that is there', () => {
        const image = [{ kind: 'image' as const, id: 'i1' }];
        expect(checkDraft('', image)).toEqual({ ok: true, text: '' });
        expect(checkDraft('  \n ', image)).toEqual({ ok: true, text: '' });
        expect(checkDraft(' look ', image)).toEqual({ ok: true, text: 'look' });
        expect(checkDraft('', [])).toEqual({ ok: false, reason: 'empty' });
        expect(checkDraft('x'.repeat(MAX_CHAT_CHARS + 1), image)).toEqual({ ok: false, reason: 'too_long' });
        expect(checkDraft('bell\u0007', image)).toEqual({ ok: false, reason: 'invalid_character' });
    });

    test('refuses what the attachment rules refuse, even with text', () => {
        const images = (n: number) => Array.from({ length: n }, (_, i) => ({ kind: 'image' as const, id: `i${i}` }));
        const gif = { kind: 'gif' as const, url: 'https://static.klipy.com/a.gif', width: 10, height: 10 };
        expect(checkDraft('hi', images(MAX_IMAGES_PER_MESSAGE)).ok).toBe(true);
        expect(checkDraft('hi', images(MAX_IMAGES_PER_MESSAGE + 1))).toEqual({ ok: false, reason: 'too_many_images' });
        expect(checkDraft('', [gif, ...images(1)])).toEqual({ ok: false, reason: 'gif_not_alone' });
        expect(checkDraft('', [...images(1), ...images(1)])).toEqual({ ok: false, reason: 'duplicate_image' });
    });

    test('counts characters, not UTF-16 units', () => {
        expect(checkDraft('😀'.repeat(MAX_CHAT_CHARS)).ok).toBe(true);
        expect(checkDraft('😀'.repeat(MAX_CHAT_CHARS + 1))).toEqual({ ok: false, reason: 'too_long' });
    });

    test('keeps line breaks and tabs and refuses other control characters', () => {
        expect(checkDraft('one\ntwo\tthree').ok).toBe(true);
        expect(checkDraft('bell\u0007')).toEqual({ ok: false, reason: 'invalid_character' });
        expect(checkDraft('nul\u0000')).toEqual({ ok: false, reason: 'invalid_character' });
    });
});
