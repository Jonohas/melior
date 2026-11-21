import { describe, expect, test } from 'vitest';
import { aspectOf } from './screen-fit';

describe('aspectOf', () => {
    test('is the width over the height of a frame', () => {
        expect(aspectOf(1920, 1080)).toBeCloseTo(16 / 9);
        expect(aspectOf(3440, 1440)).toBeCloseTo(3440 / 1440);
        expect(aspectOf(1080, 1920)).toBeCloseTo(9 / 16);
    });

    test('is null while the frame size is unknown or nonsense', () => {
        expect(aspectOf(0, 0)).toBeNull();
        expect(aspectOf(1920, 0)).toBeNull();
        expect(aspectOf(-1, 1080)).toBeNull();
        expect(aspectOf(Number.NaN, 1080)).toBeNull();
        expect(aspectOf(Number.POSITIVE_INFINITY, 1080)).toBeNull();
    });
});
