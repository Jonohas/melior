import { describe, expect, test } from 'vitest';
import { parseForceRelay } from './force-relay';

describe('parseForceRelay', () => {
    test('accepts the truthy spellings', () => {
        for (const value of [true, 1, '1', 'true']) expect(parseForceRelay(value)).toBe(true);
    });

    test('everything else is false', () => {
        for (const value of [undefined, null, false, 0, '0', 'false', '', 'yes', 2, {}]) {
            expect(parseForceRelay(value)).toBe(false);
        }
    });
});
