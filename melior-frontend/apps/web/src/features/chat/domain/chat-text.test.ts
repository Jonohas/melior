import { describe, expect, test } from 'vitest';
import { checkDraft, MAX_CHAT_CHARS } from './chat-text';

describe('checkDraft', () => {
    test('trims the text', () => {
        expect(checkDraft('  hello \n')).toEqual({ ok: true, text: 'hello' });
    });

    test('refuses an empty or blank draft', () => {
        expect(checkDraft('')).toEqual({ ok: false, reason: 'empty' });
        expect(checkDraft(' \n\t ')).toEqual({ ok: false, reason: 'empty' });
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
