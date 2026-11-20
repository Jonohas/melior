import { describe, expect, test } from 'vitest';
import { DEFAULT_CALL_PREFERENCES, parseCallPreferences } from './call-preferences';

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
