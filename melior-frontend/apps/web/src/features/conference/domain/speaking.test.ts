import { describe, expect, test } from 'vitest';
import { nextSpeaking, QUIET, rmsLevel, SPEAKING_HOLD_MS, SPEAKING_THRESHOLD } from './speaking';

const LOUD = SPEAKING_THRESHOLD * 2;
const SOFT = SPEAKING_THRESHOLD / 2;

describe('nextSpeaking', () => {
    test('a loud sample starts speaking at once', () => {
        expect(nextSpeaking(QUIET, LOUD, 1000, false)).toEqual({ speaking: true, lastLoudMs: 1000 });
    });

    test('a quiet sample keeps speaking during the hold and ends it after', () => {
        const loud = nextSpeaking(QUIET, LOUD, 1000, false);
        expect(nextSpeaking(loud, SOFT, 1000 + SPEAKING_HOLD_MS - 1, false).speaking).toBe(true);
        expect(nextSpeaking(loud, SOFT, 1000 + SPEAKING_HOLD_MS, false).speaking).toBe(false);
    });

    test('quiet without any loud sample stays quiet, and muted is never speaking', () => {
        expect(nextSpeaking(QUIET, SOFT, 5, false)).toEqual(QUIET);
        expect(nextSpeaking({ speaking: true, lastLoudMs: 1 }, LOUD, 2, true)).toEqual(QUIET);
    });
});

describe('rmsLevel', () => {
    test('is the root mean square of the samples', () => {
        expect(rmsLevel([])).toBe(0);
        expect(rmsLevel([0.5, -0.5])).toBeCloseTo(0.5);
        expect(rmsLevel([0, 0, 0, 0])).toBe(0);
    });
});
