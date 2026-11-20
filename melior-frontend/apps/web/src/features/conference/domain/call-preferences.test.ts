import { describe, expect, test } from 'vitest';
import { DEFAULT_CALL_PREFERENCES, parseCallPreferences, trackEnabled } from './call-preferences';

describe('parseCallPreferences', () => {
    test('reads both switches', () => {
        expect(parseCallPreferences('{"microphoneMuted":true,"cameraOff":false}')).toEqual({
            microphoneMuted: true,
            cameraOff: false,
        });
        expect(parseCallPreferences('{"microphoneMuted":false,"cameraOff":true}')).toEqual({
            microphoneMuted: false,
            cameraOff: true,
        });
    });

    test('a missing entry or one that does not fit means on', () => {
        expect(parseCallPreferences(null)).toEqual(DEFAULT_CALL_PREFERENCES);
        expect(parseCallPreferences('not json')).toEqual(DEFAULT_CALL_PREFERENCES);
        expect(parseCallPreferences('[]')).toEqual(DEFAULT_CALL_PREFERENCES);
        expect(parseCallPreferences('{"microphoneMuted":"yes","cameraOff":1}')).toEqual(DEFAULT_CALL_PREFERENCES);
        expect(parseCallPreferences('{}')).toEqual(DEFAULT_CALL_PREFERENCES);
    });
});

describe('trackEnabled', () => {
    test('a track follows the switch of its kind', () => {
        const muted = { microphoneMuted: true, cameraOff: false };
        expect(trackEnabled('audio', muted)).toBe(false);
        expect(trackEnabled('video', muted)).toBe(true);
        const off = { microphoneMuted: false, cameraOff: true };
        expect(trackEnabled('audio', off)).toBe(true);
        expect(trackEnabled('video', off)).toBe(false);
    });
});
