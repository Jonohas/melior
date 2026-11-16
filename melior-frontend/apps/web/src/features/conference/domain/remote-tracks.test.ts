import { describe, expect, test } from 'vitest';
import { diffTracks, reconcileTracks } from './remote-tracks';

const track = (name: string) => ({ name });

describe('reconcileTracks', () => {
    test('groups the receivers of a participant, keeping the server order', () => {
        const [t1, t2, t3] = [track('t1'), track('t2'), track('t3')];
        const receivers = new Map([
            ['1', t1],
            ['2', t2],
            ['3', t3],
        ]);
        const result = reconcileTracks(
            [
                { mid: '2', session_id: 'a' },
                { mid: '1', session_id: 'b' },
                { mid: '3', session_id: 'a' },
            ],
            receivers,
        );
        expect(result).toEqual({ a: [t2, t3], b: [t1] });
    });

    test('sections that are not listed carry nothing', () => {
        const receivers = new Map([['1', track('t1')]]);
        expect(reconcileTracks([], receivers)).toEqual({});
    });

    test('skips a listed mid that has no receiver and counts a repeated mid once', () => {
        const t1 = track('t1');
        const result = reconcileTracks(
            [
                { mid: '9', session_id: 'a' },
                { mid: '1', session_id: 'a' },
                { mid: '1', session_id: 'b' },
            ],
            new Map([['1', t1]]),
        );
        expect(result).toEqual({ a: [t1] });
    });
});

describe('diffTracks', () => {
    test('reports tracks that appear and disappear per participant', () => {
        const [t1, t2, t3] = [track('t1'), track('t2'), track('t3')];
        const diff = diffTracks({ a: [t1, t2] }, { a: [t2], b: [t3] });
        expect(diff.removed).toEqual([{ sessionId: 'a', track: t1 }]);
        expect(diff.added).toEqual([{ sessionId: 'b', track: t3 }]);
    });

    test('a reused section moves its track: removed from the old participant, added to the new', () => {
        const reused = track('reused');
        const diff = diffTracks({ a: [reused] }, { b: [reused] });
        expect(diff.removed).toEqual([{ sessionId: 'a', track: reused }]);
        expect(diff.added).toEqual([{ sessionId: 'b', track: reused }]);
    });

    test('no change gives an empty diff', () => {
        const t1 = track('t1');
        expect(diffTracks({ a: [t1] }, { a: [t1] })).toEqual({ removed: [], added: [] });
    });
});
