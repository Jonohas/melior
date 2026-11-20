import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { FakeMediaStream, FakePeerConnection, stubRtc } from './fake-rtc';
import { createPeerSession, type PeerSession } from './peer-session';

const track = (id: string, kind: string) => ({ id, kind });
const screenStream = (id: string) => new FakeMediaStream([track(id, 'video')]) as unknown as MediaStream;

describe('screen sharing on the server connection', () => {
    let session: PeerSession;
    let connection: FakePeerConnection;

    beforeEach(() => {
        stubRtc();
        session = createPeerSession({
            iceServers: [],
            localStream: new FakeMediaStream([track('mic', 'audio'), track('cam', 'video')]) as unknown as MediaStream,
            onIceCandidate: () => undefined,
            onOffer: () => undefined,
            onRemoteMedia: () => undefined,
            onConnected: () => undefined,
            onFailed: () => undefined,
        });
        const found = FakePeerConnection.instances[0];
        if (!found) throw new Error('no connection');
        connection = found;
        // Sections the server made to forward other participants' media: idle, never sent, video.
        connection.addTransceiver('video', { direction: 'recvonly' });
        connection.addTransceiver('video', { direction: 'recvonly' });
    });

    afterEach(() => {
        session.close();
        vi.unstubAllGlobals();
    });

    const sending = () => connection.transceivers.filter((t) => t.sender.track?.id.startsWith('scr'));

    test('the screen never lands on a section the server created', () => {
        const serverSections = connection.transceivers.slice(2);
        session.shareScreen(screenStream('scr1'));
        expect(sending()).toHaveLength(1);
        expect(serverSections.every((t) => t.sender.track === null && t.direction === 'recvonly')).toBe(true);
        expect(sending()[0]?.direction).toBe('sendonly');
    });

    test('stopping ends the track on the connection and a second share publishes again, on a section of its own', () => {
        session.shareScreen(screenStream('scr1'));
        const first = sending()[0];
        session.stopScreen();
        expect(first?.sender.track).toBeNull();
        expect(first?.direction).toBe('recvonly');
        session.shareScreen(screenStream('scr2'));
        const second = sending();
        expect(second).toHaveLength(1);
        expect(second[0]).not.toBe(first);
        expect(second[0]?.sender.track?.id).toBe('scr2');
        expect(connection.transceivers.slice(2, 4).every((t) => t.sender.track === null)).toBe(true);
    });

    test('a share that is already running is not added twice', () => {
        session.shareScreen(screenStream('scr1'));
        session.shareScreen(screenStream('scr1'));
        expect(connection.transceivers.filter((t) => t.sender.track?.id === 'scr1')).toHaveLength(1);
    });
});
