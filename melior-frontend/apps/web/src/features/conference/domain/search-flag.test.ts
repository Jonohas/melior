import { describe, expect, test } from 'vitest';
import { parseSearchFlag } from './search-flag';

describe('parseSearchFlag', () => {
    test('accepts the truthy spellings', () => {
        for (const value of [true, 1, '1', 'true']) expect(parseSearchFlag(value)).toBe(true);
    });

    test('everything else is false', () => {
        for (const value of [undefined, null, false, 0, '0', 'false', '', 'yes', 2, {}]) {
            expect(parseSearchFlag(value)).toBe(false);
        }
    });
});
