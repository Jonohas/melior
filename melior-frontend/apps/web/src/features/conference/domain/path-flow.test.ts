import { describe, expect, test } from 'vitest';
import { countInbound, FLOW_WINDOW_MS, flowingKinds, updateFlow } from './path-flow';

describe('countInbound', () => {
    test('maps entries to peers through their mid and sums per kind', () => {
        const peers: Record<string, string> = { '0': 'a', '1': 'a', '2': 'b' };
        const result = countInbound(
            [
                { kind: 'video', mid: '0', framesDecoded: 10 },
                { kind: 'audio', mid: '1', packetsReceived: 7 },
                { kind: 'video', mid: '2', framesDecoded: 3 },
                { kind: 'video', mid: '9', framesDecoded: 99 },
                { kind: 'video', mid: null, framesDecoded: 99 },
            ],
            (mid) => (mid ? peers[mid] : undefined),
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
