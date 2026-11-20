import { describe, expect, test } from 'vitest';
import { parseScreenState, screenStateMessage, sharingPeers } from './screen-state';

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

describe('sharingPeers', () => {
    const stream = {};

    test('lists a screen that exists on the path its sharer is shown over', () => {
        expect(sharingPeers({ a: stream }, {}, {})).toEqual(['a']);
        expect(sharingPeers({}, { b: stream }, { b: 'direct' })).toEqual(['b']);
    });

    test('a screen that exists only on the other path waits, so no tile is empty', () => {
        expect(sharingPeers({ a: stream }, {}, { a: 'direct' })).toEqual([]);
        expect(sharingPeers({}, { b: stream }, {})).toEqual([]);
        expect(sharingPeers({}, { b: stream }, { b: 'forwarded' })).toEqual([]);
    });

    test('lists a peer once when both paths carry its screen', () => {
        expect(sharingPeers({ a: stream }, { a: stream }, { a: 'direct' })).toEqual(['a']);
    });
});
