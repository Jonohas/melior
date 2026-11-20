import { describe, expect, test } from 'vitest';
import { diffTracks, parseStreamKey, partitionStreams, reconcileTracks, streamKey } from './remote-tracks';

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
                { mid: '2', session_id: 'a', source: 'camera' },
                { mid: '1', session_id: 'b', source: 'camera' },
                { mid: '3', session_id: 'a', source: 'camera' },
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
                { mid: '9', session_id: 'a', source: 'camera' },
                { mid: '1', session_id: 'a', source: 'camera' },
                { mid: '1', session_id: 'b', source: 'camera' },
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

describe('stream keys', () => {
    test('a camera is named by the session id and a screen has a name of its own', () => {
        expect(streamKey('a', 'camera')).toBe('a');
        expect(streamKey('a', 'screen')).not.toBe('a');
    });

    test('a key splits back into the participant and the source', () => {
        expect(parseStreamKey(streamKey('abc-1', 'camera'))).toEqual({ sessionId: 'abc-1', source: 'camera' });
        expect(parseStreamKey(streamKey('abc-1', 'screen'))).toEqual({ sessionId: 'abc-1', source: 'screen' });
    });

    test('streams are partitioned into cameras and screens by participant', () => {
        const [camera, screen, other] = [{ n: 1 }, { n: 2 }, { n: 3 }];
        expect(
            partitionStreams({ [streamKey('a', 'camera')]: camera, [streamKey('a', 'screen')]: screen, b: other }),
        ).toEqual({ camera: { a: camera, b: other }, screen: { a: screen } });
    });
});

describe('reconcileTracks with screens', () => {
    test('a participant who shares a screen has two streams', () => {
        const [cam, scr] = [track('cam'), track('scr')];
        const result = reconcileTracks(
            [
                { mid: '0', session_id: 'a', source: 'camera' },
                { mid: '1', session_id: 'a', source: 'screen' },
            ],
            new Map([
                ['0', cam],
                ['1', scr],
            ]),
        );
        expect(result).toEqual({ [streamKey('a', 'camera')]: [cam], [streamKey('a', 'screen')]: [scr] });
    });

    test('a reused section that changes from a screen to a camera moves between streams', () => {
        const reused = track('reused');
        const before = reconcileTracks([{ mid: '1', session_id: 'a', source: 'screen' }], new Map([['1', reused]]));
        const after = reconcileTracks([{ mid: '1', session_id: 'b', source: 'camera' }], new Map([['1', reused]]));
        const diff = diffTracks(before, after);
        expect(diff.removed).toEqual([{ sessionId: streamKey('a', 'screen'), track: reused }]);
        expect(diff.added).toEqual([{ sessionId: 'b', track: reused }]);
    });
});
