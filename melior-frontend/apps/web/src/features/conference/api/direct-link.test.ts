import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import type { ClientMessage } from '@/features/conference/domain';
import { createDirectLink, type DirectLink } from './direct-link';
import type { LocalTracks } from './direct-sections';
import { FakePeerConnection, stubRtc } from './fake-rtc';

const NO_LOCAL_TRACKS: LocalTracks = { audio: null, video: null, screen: null, screenAudio: null };

describe('createDirectLink local candidates', () => {
    let sent: ClientMessage[];

    const link = (offerer: boolean): DirectLink =>
        createDirectLink({
            peerId: 'peer',
            offerer,
            iceServers: [],
            forceRelay: false,
            localStream: null,
            local: () => NO_LOCAL_TRACKS,
            send: (message) => sent.push(message),
            onMedia: () => undefined,
            onScreen: () => undefined,
            onFailed: () => undefined,
        });

    beforeEach(() => {
        stubRtc();
        sent = [];
    });

    afterEach(() => {
        vi.unstubAllGlobals();
    });

    test('a candidate fired while the offer is being set goes out after the offer', async () => {
        const offerer = link(true);
        const connection = FakePeerConnection.instances[0];
        if (!connection) throw new Error('no connection');
        connection.duringSetLocalDescription = () => connection.fireCandidate('early');
        await offerer.start();
        connection.fireCandidate('late');
        expect(sent.map((message) => message.channel)).toEqual(['peer_offer', 'peer_ice', 'peer_ice']);
        expect(
            sent.slice(1).map((message) => message.channel === 'peer_ice' && message.payload.candidate.candidate),
        ).toEqual(['early', 'late']);
    });

    test('the answerer holds candidates until its answer is sent', async () => {
        const answerer = link(false);
        const connection = FakePeerConnection.instances[0];
        if (!connection) throw new Error('no connection');
        connection.duringSetLocalDescription = () => connection.fireCandidate('early');
        await answerer.handleOffer('offer');
        expect(sent.map((message) => message.channel)).toEqual(['peer_answer', 'peer_ice']);
    });
});

describe('createDirectLink sender settings', () => {
    const SCREEN = { id: 'screen', kind: 'video' };
    const CAMERA = { id: 'camera', kind: 'video' };

    beforeEach(() => {
        stubRtc();
    });

    afterEach(() => {
        vi.unstubAllGlobals();
    });

    test('after the answer, the screen keeps its resolution and the camera is left to congestion control', async () => {
        const offerer = createDirectLink({
            peerId: 'peer',
            offerer: true,
            iceServers: [],
            forceRelay: false,
            localStream: null,
            local: () => ({ audio: null, video: CAMERA, screen: SCREEN }) as unknown as LocalTracks,
            send: () => undefined,
            onMedia: () => undefined,
            onScreen: () => undefined,
            onFailed: () => undefined,
        });
        const connection = FakePeerConnection.instances[0];
        if (!connection) throw new Error('no connection');
        await offerer.start();
        await offerer.handleAnswer('answer');
        await vi.waitFor(() =>
            expect(connection.transceivers[2]?.sender.degradationPreference).toBe('maintain-resolution'),
        );
        expect(connection.transceivers[1]?.sender.getParameters()).toEqual({ encodings: [{ active: true }] });
        offerer.close();
    });
});
