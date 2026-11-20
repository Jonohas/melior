import { describe, expect, test } from 'vitest';
import { parseScreenState, screenStateMessage } from './screen-state';

describe('parseScreenState', () => {
    test('reads the sharing state a peer sends', () => {
        expect(parseScreenState('{"screen":true}')).toBe(true);
        expect(parseScreenState('{"screen":false}')).toBe(false);
        expect(parseScreenState(screenStateMessage(true))).toBe(true);
        expect(parseScreenState(screenStateMessage(false))).toBe(false);
    });

    test('ignores everything else', () => {
        for (const hostile of [
            'not json',
            '',
            'null',
            '[]',
            '"screen"',
            '{}',
            '{"screen":"true"}',
            '{"screen":1}',
            '{"screen":null}',
            new ArrayBuffer(4),
            undefined,
            42,
        ]) {
            expect(parseScreenState(hostile)).toBeNull();
        }
    });
});
