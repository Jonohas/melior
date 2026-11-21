import { describe, expect, test } from 'vitest';
import { countInbound, directKindOf, FLOW_WINDOW_MS, flowingKinds, sfuSourceOf, updateFlow } from './path-flow';

describe('sfuSourceOf', () => {
    const sections = [
        { mid: '0', trackId: 't0' },
        { mid: '1', trackId: 't1' },
        { mid: null, trackId: 't2' },
    ];
    const assignments = [
        { mid: '0', session_id: 'a', source: 'camera' as const },
        { mid: '1', session_id: 'a', source: 'screen' as const },
        { mid: '2', session_id: 'b', source: 'camera' as const },
    ];
    const video = (trackIdentifier: string | undefined) => ({ kind: 'video', trackIdentifier });

    test('follows track id to mid to publisher and kind', () => {
        expect(sfuSourceOf(video('t0'), sections, assignments)).toEqual({ peer: 'a', kind: 'video' });
        expect(sfuSourceOf({ kind: 'audio', trackIdentifier: 't0' }, sections, assignments)).toEqual({
            peer: 'a',
            kind: 'audio',
        });
    });

    test("a shared screen is the peer's screen kind, never its camera video", () => {
        expect(sfuSourceOf(video('t1'), sections, assignments)).toEqual({ peer: 'a', kind: 'screen' });
    });

    test('is undefined when a link of the chain is missing', () => {
        expect(sfuSourceOf(video('t2'), sections, assignments)).toBeUndefined();
        expect(sfuSourceOf(video('nope'), sections, assignments)).toBeUndefined();
        expect(sfuSourceOf(video(undefined), sections, assignments)).toBeUndefined();
        expect(sfuSourceOf({ kind: 'application', trackIdentifier: 't0' }, sections, assignments)).toBeUndefined();
    });
});

describe('directKindOf', () => {
    test('reads the kind from the position of the receiving track among the sections', () => {
        const ids = ['ta', 'tc', 'ts'];
        expect(directKindOf({ kind: 'audio', trackIdentifier: 'ta' }, ids)).toBe('audio');
        expect(directKindOf({ kind: 'video', trackIdentifier: 'tc' }, ids)).toBe('video');
        expect(directKindOf({ kind: 'video', trackIdentifier: 'ts' }, ids)).toBe('screen');
        expect(directKindOf({ kind: 'video', trackIdentifier: 'other' }, ids)).toBeUndefined();
    });

    test("the screen's sound is no kind: it decides nothing about the path", () => {
        const ids = ['ta', 'tc', 'ts', 'tsa'];
        expect(directKindOf({ kind: 'audio', trackIdentifier: 'tsa' }, ids)).toBeUndefined();
    });
});

describe('countInbound', () => {
    test('maps entries to peers and sums per kind; a screen does not count as the camera', () => {
        const sources: Record<string, { peer: string; kind: 'audio' | 'video' | 'screen' }> = {
            t0: { peer: 'a', kind: 'video' },
            t1: { peer: 'a', kind: 'audio' },
            t2: { peer: 'b', kind: 'video' },
            t3: { peer: 'a', kind: 'screen' },
        };
        const result = countInbound(
            [
                { kind: 'video', trackIdentifier: 't0', framesDecoded: 10 },
                { kind: 'audio', trackIdentifier: 't1', packetsReceived: 7 },
                { kind: 'video', trackIdentifier: 't2', framesDecoded: 3 },
                { kind: 'video', trackIdentifier: 't3', framesDecoded: 40 },
                { kind: 'video', trackIdentifier: 't9', framesDecoded: 99 },
                { kind: 'video', framesDecoded: 99 },
            ],
            (entry) => (entry.trackIdentifier ? sources[entry.trackIdentifier] : undefined),
        );
        expect(result.get('a')).toEqual({ video: 10, audio: 7, screen: 40 });
        expect(result.get('b')).toEqual({ video: 3, audio: 0, screen: 0 });
        expect(result.size).toBe(2);
    });
});

describe('flow tracking', () => {
    test('the first sample is a baseline, growth makes a kind flow for the window', () => {
        let state = updateFlow(undefined, { audio: 5, video: 5, screen: 0 }, 0);
        expect(flowingKinds(state, 0)).toEqual([]);
        state = updateFlow(state, { audio: 9, video: 5, screen: 0 }, 250);
        expect(flowingKinds(state, 250)).toEqual(['audio']);
        expect(flowingKinds(state, 250 + FLOW_WINDOW_MS)).toEqual(['audio']);
        expect(flowingKinds(state, 251 + FLOW_WINDOW_MS)).toEqual([]);
    });

    test('a counter that stays still stops flowing', () => {
        let state = updateFlow(undefined, { audio: 0, video: 0, screen: 0 }, 0);
        state = updateFlow(state, { audio: 1, video: 1, screen: 0 }, 100);
        state = updateFlow(state, { audio: 1, video: 2, screen: 0 }, 1300);
        expect(flowingKinds(state, 1300)).toEqual(['video']);
    });
});

describe('flow tracking of a screen', () => {
    test('the screen kind flows on its own counter', () => {
        let state = updateFlow(undefined, { audio: 0, video: 0, screen: 0 }, 0);
        state = updateFlow(state, { audio: 0, video: 0, screen: 3 }, 250);
        expect(flowingKinds(state, 250)).toEqual(['screen']);
    });
});
