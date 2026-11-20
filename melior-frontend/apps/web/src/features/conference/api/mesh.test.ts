import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import type { ClientMessage } from '@/features/conference/domain';
import { FakePeerConnection, stubRtc } from './fake-rtc';
import { createMesh, type Mesh } from './mesh';

// 'a' sorts below 'b', so this side offers.
const SELF = 'a';
const PEER = 'b';

describe('createMesh', () => {
    let sent: ClientMessage[];
    let media: Array<[string, unknown]>;
    let shown: Array<[string, string]>;
    let mesh: Mesh;

    const route = (path: 'direct' | 'forwarded', epoch: number) =>
        mesh.handle({ channel: 'route', payload: { peer: PEER, path, epoch } });
    const sentChannels = (channel: string) => sent.filter((message) => message.channel === channel);

    beforeEach(() => {
        vi.useFakeTimers();
        stubRtc();
        sent = [];
        media = [];
        shown = [];
        mesh = createMesh({
            selfId: SELF,
            iceServers: [],
            forceRelay: false,
            localStream: null,
            sfu: () => null,
            send: (message) => sent.push(message),
            onDirectMedia: (peer, stream) => media.push([peer, stream]),
            onShownPath: (peer, path) => shown.push([peer, path]),
        });
    });

    afterEach(() => {
        mesh.close();
        vi.useRealTimers();
        vi.unstubAllGlobals();
    });

    test('route direct opens one link and sends one offer', async () => {
        await route('direct', 1);
        expect(FakePeerConnection.instances).toHaveLength(1);
        expect(sentChannels('peer_offer')).toHaveLength(1);
    });

    test('a repeated route direct on the same link does not negotiate again', async () => {
        await route('direct', 1);
        await route('forwarded', 2);
        await route('direct', 3);
        expect(FakePeerConnection.instances).toHaveLength(1);
        expect(FakePeerConnection.instances[0]?.offersCreated).toBe(1);
        expect(sentChannels('peer_offer')).toHaveLength(1);
    });

    test('the answering side does not offer', async () => {
        const answerer = createMesh({
            selfId: 'c',
            iceServers: [],
            forceRelay: false,
            localStream: null,
            sfu: () => null,
            send: (message) => sent.push(message),
            onDirectMedia: () => undefined,
            onShownPath: () => undefined,
        });
        await answerer.handle({ channel: 'route', payload: { peer: PEER, path: 'direct', epoch: 1 } });
        expect(sentChannels('peer_offer')).toHaveLength(0);
        await answerer.handle({ channel: 'peer_offer', payload: { from: PEER, sdp: 'offer' } });
        expect(sentChannels('peer_answer')).toHaveLength(1);
        answerer.close();
    });

    test('link_close closes the link and drops its media', async () => {
        await route('direct', 1);
        await mesh.handle({ channel: 'link_close', payload: { peer: PEER } });
        expect(FakePeerConnection.instances[0]?.closed).toBe(true);
        expect(media).toContainEqual([PEER, null]);
    });

    test('peer_left closes the link', async () => {
        await route('direct', 1);
        mesh.peerLeft(PEER);
        expect(FakePeerConnection.instances[0]?.closed).toBe(true);
        expect(media).toContainEqual([PEER, null]);
    });

    test('a failed link is closed and reported once', async () => {
        await route('direct', 1);
        const connection = FakePeerConnection.instances[0];
        connection?.setState('failed');
        connection?.setState('failed');
        expect(connection?.closed).toBe(true);
        expect(sentChannels('link_failed')).toEqual([{ channel: 'link_failed', payload: { peer: PEER } }]);
        expect(shown.every(([, path]) => path === 'forwarded')).toBe(true);
    });

    test('a failed link of a peer shown over direct switches the tile back to forwarded', async () => {
        await route('direct', 1);
        const connection = FakePeerConnection.instances[0];
        connection?.setState('connected');
        connection?.statsEntries.push({ type: 'inbound-rtp', kind: 'video', trackIdentifier: 't', framesDecoded: 1 });
        // The direct link flows; the forwarded path is not sampled (no SFU), so only direct is observed.
        await vi.advanceTimersByTimeAsync(300);
        const entry = connection?.statsEntries[0];
        if (entry) entry.framesDecoded = 5;
        await vi.advanceTimersByTimeAsync(300);
        expect(shown).toContainEqual([PEER, 'direct']);
        connection?.setState('failed');
        expect(shown.at(-1)).toEqual([PEER, 'forwarded']);
        expect(sentChannels('link_failed')).toHaveLength(1);
    });

    test('close closes every link', async () => {
        await route('direct', 1);
        mesh.close();
        expect(FakePeerConnection.instances.every((connection) => connection.closed)).toBe(true);
    });
});
