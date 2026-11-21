import { describe, expect, test } from 'vitest';
import { redFirst } from './audio-codecs';

const codec = (mimeType: string, fmtp = '') => ({ mimeType, sdpFmtpLine: fmtp });

describe('redFirst', () => {
    test('moves RED ahead of Opus and keeps the rest in order', () => {
        const codecs = [codec('audio/opus'), codec('audio/red', '111/111'), codec('audio/G722'), codec('audio/CN')];
        expect(redFirst(codecs)?.map((c) => c.mimeType)).toEqual(['audio/red', 'audio/opus', 'audio/G722', 'audio/CN']);
    });

    test('matches the MIME type in any case', () => {
        expect(redFirst([codec('audio/opus'), codec('AUDIO/RED')])?.[0]?.mimeType).toBe('AUDIO/RED');
    });

    test('says nothing when the browser has no RED', () => {
        expect(redFirst([codec('audio/opus'), codec('audio/PCMU')])).toBeUndefined();
        expect(redFirst([])).toBeUndefined();
    });
});
