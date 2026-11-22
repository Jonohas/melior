import { describe, expect, test } from 'vitest';
import { bitrateBetween, type StatsEntry, stateChanges, summarizeTransport } from './debug-stats';

const LOCAL = {
    id: 'L1',
    type: 'local-candidate',
    candidateType: 'srflx',
    protocol: 'udp',
    address: '203.0.113.9',
    port: 40000,
};
const REMOTE = {
    id: 'R1',
    type: 'remote-candidate',
    candidateType: 'host',
    protocol: 'udp',
    address: '10.0.0.2',
    port: 5000,
};
const RELAY = {
    id: 'L2',
    type: 'local-candidate',
    candidateType: 'relay',
    protocol: 'udp',
    ip: '198.51.100.2',
    port: 3478,
    relayProtocol: 'tls',
};

const pair = (id: string, extra: StatsEntry): StatsEntry => ({
    id,
    type: 'candidate-pair',
    localCandidateId: 'L1',
    remoteCandidateId: 'R1',
    state: 'succeeded',
    bytesSent: 100,
    bytesReceived: 200,
    ...extra,
});

describe('summarizeTransport', () => {
    test('the transport names the pair in use (Chromium) and carries the bytes', () => {
        const entries = [
            LOCAL,
            REMOTE,
            pair('P0', { localCandidateId: 'L2', state: 'failed' }),
            pair('P1', { currentRoundTripTime: 0.0423 }),
            { id: 'T1', type: 'transport', selectedCandidatePairId: 'P1', bytesSent: 1000, bytesReceived: 5000 },
        ];
        expect(summarizeTransport(entries)).toEqual({
            selectedPair: {
                local: { type: 'srflx', protocol: 'udp', address: '203.0.113.9', port: 40000, relayProtocol: null },
                remote: { type: 'host', protocol: 'udp', address: '10.0.0.2', port: 5000, relayProtocol: null },
                state: 'succeeded',
                rttMs: 42,
            },
            bytesSent: 1000,
            bytesReceived: 5000,
        });
    });

    test('without a transport, the pair marked selected (Firefox) gives the pair and the bytes', () => {
        const entries = [
            RELAY,
            REMOTE,
            pair('P0', { selected: false }),
            pair('P1', { localCandidateId: 'L2', selected: true }),
        ];
        const summary = summarizeTransport(entries);
        expect(summary.selectedPair?.local).toEqual({
            type: 'relay',
            protocol: 'udp',
            address: '198.51.100.2',
            port: 3478,
            relayProtocol: 'tls',
        });
        expect(summary.bytesSent).toBe(100);
        expect(summary.bytesReceived).toBe(200);
    });

    test('a nominated pair that succeeded counts when nothing is marked', () => {
        const entries = [LOCAL, REMOTE, pair('P0', { state: 'in-progress' }), pair('P1', { nominated: true })];
        expect(summarizeTransport(entries).selectedPair?.state).toBe('succeeded');
    });

    test('a connection still checking has no pair and no bytes', () => {
        const entries = [LOCAL, pair('P0', { state: 'in-progress', bytesSent: 0, bytesReceived: 0 })];
        expect(summarizeTransport(entries)).toEqual({ selectedPair: null, bytesSent: 0, bytesReceived: 0 });
        expect(summarizeTransport([])).toEqual({ selectedPair: null, bytesSent: 0, bytesReceived: 0 });
    });
});

describe('bitrateBetween', () => {
    test('kilobits per second between two samples', () => {
        const previous = { at: 1000, bytesSent: 0, bytesReceived: 0 };
        const next = { at: 2000, bytesSent: 12_500, bytesReceived: 125_000 };
        expect(bitrateBetween(previous, next)).toEqual({ inKbps: 1000, outKbps: 100 });
    });

    test('nothing without a previous sample or time between them', () => {
        const sample = { at: 1000, bytesSent: 1, bytesReceived: 1 };
        expect(bitrateBetween(undefined, sample)).toBeNull();
        expect(bitrateBetween(sample, sample)).toBeNull();
    });

    test('a counter that went back (a new connection) is not negative', () => {
        const previous = { at: 0, bytesSent: 5000, bytesReceived: 5000 };
        expect(bitrateBetween(previous, { at: 1000, bytesSent: 0, bytesReceived: 0 })).toEqual({
            inKbps: 0,
            outKbps: 0,
        });
    });
});

describe('stateChanges', () => {
    const states = { connectionState: 'connecting', iceConnectionState: 'checking', iceGatheringState: 'gathering' };

    test('the first sample states everything', () => {
        expect(stateChanges(undefined, states)).toEqual([
            'connectionState connecting',
            'iceConnectionState checking',
            'iceGatheringState gathering',
        ]);
    });

    test('later samples name only what changed', () => {
        expect(stateChanges(states, states)).toEqual([]);
        expect(stateChanges(states, { ...states, iceConnectionState: 'failed', connectionState: 'failed' })).toEqual([
            'connectionState connecting → failed',
            'iceConnectionState checking → failed',
        ]);
    });
});
