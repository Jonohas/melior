import { describe, expect, test } from 'vitest';
import { countInbound, FLOW_WINDOW_MS, flowingKinds, peerForTrack, updateFlow } from './path-flow';

describe('peerForTrack', () => {
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

    test('follows track id to mid to publisher', () => {
        expect(peerForTrack('t0', sections, assignments)).toBe('a');
    });

    test("a shared screen belongs to no peer, so it cannot count as the peer's video", () => {
        expect(peerForTrack('t1', sections, assignments)).toBeUndefined();
    });

    test('is undefined when a link of the chain is missing', () => {
        expect(peerForTrack('t2', sections, assignments)).toBeUndefined();
        expect(peerForTrack('nope', sections, assignments)).toBeUndefined();
        expect(peerForTrack(undefined, sections, assignments)).toBeUndefined();
    });
});

describe('countInbound', () => {
    test('maps entries to peers and sums per kind', () => {
        const peers: Record<string, string> = { t0: 'a', t1: 'a', t2: 'b' };
        const result = countInbound(
            [
                { kind: 'video', trackIdentifier: 't0', framesDecoded: 10 },
                { kind: 'audio', trackIdentifier: 't1', packetsReceived: 7 },
                { kind: 'video', trackIdentifier: 't2', framesDecoded: 3 },
                { kind: 'video', trackIdentifier: 't9', framesDecoded: 99 },
                { kind: 'video', framesDecoded: 99 },
            ],
            (entry) => (entry.trackIdentifier ? peers[entry.trackIdentifier] : undefined),
        );
        expect(result.get('a')).toEqual({ video: 10, audio: 7 });
        expect(result.get('b')).toEqual({ video: 3, audio: 0 });
        expect(result.size).toBe(2);
    });
});

describe('flow tracking', () => {
    test('the first sample is a baseline, growth makes a kind flow for the window', () => {
        let state = updateFlow(undefined, { audio: 5, video: 5 }, 0);
        expect(flowingKinds(state, 0)).toEqual([]);
        state = updateFlow(state, { audio: 9, video: 5 }, 250);
        expect(flowingKinds(state, 250)).toEqual(['audio']);
        expect(flowingKinds(state, 250 + FLOW_WINDOW_MS)).toEqual(['audio']);
        expect(flowingKinds(state, 251 + FLOW_WINDOW_MS)).toEqual([]);
    });

    test('a counter that stays still stops flowing', () => {
        let state = updateFlow(undefined, { audio: 0, video: 0 }, 0);
        state = updateFlow(state, { audio: 1, video: 1 }, 100);
        state = updateFlow(state, { audio: 1, video: 2 }, 1300);
        expect(flowingKinds(state, 1300)).toEqual(['video']);
    });
});
