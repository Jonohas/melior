import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { initialRouteView } from '@/features/conference/domain';
import { createCallRecorder } from './call-recorder';
import { FakePeerConnection, stubRtc } from './fake-rtc';
import type { SignallingClient } from './signalling-client';

const BOB = 'bob-session';

const asConnection = (fake: FakePeerConnection) => fake as unknown as RTCPeerConnection;

describe('createCallRecorder', () => {
    let clock: number;
    let sfu: FakePeerConnection;

    beforeEach(() => {
        stubRtc();
        clock = 0;
        sfu = new FakePeerConnection();
    });

    afterEach(() => {
        vi.unstubAllGlobals();
    });

    test('a sample logs the states once, then only their changes, and computes the bitrate', async () => {
        const recorder = createCallRecorder(() => clock);
        sfu.statsEntries = [{ id: 'T', type: 'transport', bytesSent: 0, bytesReceived: 0 }];
        await recorder.sample({ forceRelay: false, sfu: asConnection(sfu), links: [] });
        clock = 1000;
        sfu.iceConnectionState = 'checking';
        sfu.statsEntries = [{ id: 'T', type: 'transport', bytesSent: 1250, bytesReceived: 12_500 }];
        const snapshot = await recorder.sample({ forceRelay: false, sfu: asConnection(sfu), links: [] });
        expect(snapshot.log.map((line) => line.text)).toEqual([
            'SFU connectionState new',
            'SFU iceConnectionState new',
            'SFU iceGatheringState new',
            'SFU iceConnectionState new → checking',
        ]);
        expect(snapshot.sfu?.bitrate).toEqual({ inKbps: 100, outKbps: 10 });
    });

    test('the tapped client records what it sent, and only what it sent', () => {
        const recorder = createCallRecorder(() => clock);
        let open = true;
        const client: SignallingClient = { send: () => open, close: () => undefined };
        const tapped = recorder.tap(client);
        expect(tapped.send({ channel: 'link_failed', payload: { peer: BOB } })).toBe(true);
        open = false;
        expect(tapped.send({ channel: 'link_failed', payload: { peer: BOB } })).toBe(false);
        return recorder.sample({ forceRelay: false, sfu: null, links: [] }).then((snapshot) => {
            expect(snapshot.log.map((line) => line.text)).toEqual(['link_failed unknown peer']);
        });
    });

    test('a direct link is sampled under its peer label; a peer without a link shows none', async () => {
        const recorder = createCallRecorder(() => clock);
        recorder.received({
            channel: 'peer_joined',
            payload: {
                session_id: BOB,
                username: 'bob',
                voice: { muted: false, camera: true, screen: false, speaking: false },
            },
        });
        const link = new FakePeerConnection();
        const view = { ...initialRouteView(), preferred: 'direct' as const, epoch: 1 };
        const snapshot = await recorder.sample({
            forceRelay: true,
            sfu: null,
            links: [{ peer: BOB, view, connection: asConnection(link) }],
        });
        expect(snapshot.peers[0]).toMatchObject({
            peer: 'bob #1',
            route: 'ToDirect',
            link: { connectionState: 'new' },
        });
        expect(snapshot.log.map((line) => line.text)).toContain('direct link to bob #1 iceConnectionState new');
        expect(snapshot.room.forceRelay).toBe(true);
    });

    test('reset forgets the call', async () => {
        const recorder = createCallRecorder(() => clock);
        recorder.received({ channel: 'uplink', payload: { active: false } });
        recorder.reset();
        const snapshot = await recorder.sample({ forceRelay: false, sfu: null, links: [] });
        expect(snapshot.log).toEqual([]);
        expect(snapshot.room.uplink).toBe('sending');
    });
});
