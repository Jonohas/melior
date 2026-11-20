import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { createDirectLink, type DirectLink } from './direct-link';
import type { LocalTracks } from './direct-sections';
import { FakeMediaStream, FakePeerConnection, FakeTransceiver, stubRtc } from './fake-rtc';

const track = (id: string, kind: string) => ({ id, kind }) as unknown as MediaStreamTrack;
const MIC = track('mic', 'audio');
const CAMERA = track('cam', 'video');
const SCREEN = track('scr', 'video');

describe('the sections of a direct link', () => {
    let local: LocalTracks;
    let peerScreens: Array<unknown>;
    let peerMedia: unknown[];

    const link = (offerer: boolean): DirectLink =>
        createDirectLink({
            peerId: 'peer',
            offerer,
            iceServers: [],
            forceRelay: false,
            localStream: new FakeMediaStream([MIC, CAMERA]) as unknown as MediaStream,
            local: () => local,
            send: () => undefined,
            onMedia: (stream) => peerMedia.push(stream),
            onScreen: (stream) => peerScreens.push(stream),
            onFailed: () => undefined,
        });

    const connection = (): FakePeerConnection => {
        const found = FakePeerConnection.instances[0];
        if (!found) throw new Error('no connection');
        return found;
    };
    const sections = (): FakeTransceiver[] => connection().transceivers;
    const channel = () => {
        const found = connection().dataChannels[0];
        if (!found) throw new Error('no data channel');
        return found;
    };

    beforeEach(() => {
        stubRtc();
        local = { audio: MIC, video: CAMERA, screen: null };
        peerScreens = [];
        peerMedia = [];
    });

    afterEach(() => {
        vi.unstubAllGlobals();
    });

    test('both sides create the negotiated data channel before any negotiation', () => {
        link(true);
        link(false);
        for (const instance of FakePeerConnection.instances) {
            expect(instance.dataChannels).toHaveLength(1);
            expect(instance.dataChannels[0]?.label).toBe('melior');
            expect(instance.dataChannels[0]?.init).toEqual({ negotiated: true, id: 0 });
        }
    });

    test('the offerer has audio, camera and screen sections in that order, and negotiates once', async () => {
        const offerer = link(true);
        await offerer.start();
        await offerer.start();
        expect(sections().map((t) => [t.receiver.track.kind, t.sender.track?.id ?? null, t.direction])).toEqual([
            ['audio', 'mic', 'sendrecv'],
            ['video', 'cam', 'sendrecv'],
            ['video', null, 'sendrecv'],
        ]);
        expect(connection().offersCreated).toBe(1);
    });

    test('an offerer without a camera still has all three sections', async () => {
        local = { audio: MIC, video: null, screen: null };
        await link(true).start();
        expect(sections().map((t) => t.receiver.track.kind)).toEqual(['audio', 'video', 'video']);
    });

    test('a link opened during a share starts with the screen on its screen section', async () => {
        local = { ...local, screen: SCREEN };
        await link(true).start();
        expect(sections()[2]?.sender.track).toBe(SCREEN);
    });

    test('the answerer attaches its tracks to the offer sections and sends on all three', async () => {
        local = { ...local, screen: SCREEN };
        const answerer = link(false);
        await answerer.handleOffer('offer');
        expect(sections().map((t) => [t.sender.track?.id, t.direction])).toEqual([
            ['mic', 'sendrecv'],
            ['cam', 'sendrecv'],
            ['scr', 'sendrecv'],
        ]);
        expect(connection().tracksAdded).toBe(0);
    });

    test('the answerer without a screen leaves its screen section empty, still sending direction', async () => {
        await link(false).handleOffer('offer');
        expect(sections()[2]?.sender.track).toBeNull();
        expect(sections()[2]?.direction).toBe('sendrecv');
    });

    test('starting and stopping a share replaces the screen track and tells the peer, without renegotiating', async () => {
        const offerer = link(true);
        await offerer.start();
        channel().open();
        expect(channel().sent).toEqual(['{"screen":false}']);
        local = { ...local, screen: SCREEN };
        await offerer.setScreen(SCREEN);
        expect(sections()[2]?.sender.track).toBe(SCREEN);
        local = { ...local, screen: null };
        await offerer.setScreen(null);
        expect(sections()[2]?.sender.track).toBeNull();
        expect(channel().sent).toEqual(['{"screen":false}', '{"screen":true}', '{"screen":false}']);
        expect(connection().offersCreated).toBe(1);
    });

    test('a sharer announces itself when the channel opens', async () => {
        local = { ...local, screen: SCREEN };
        const answerer = link(false);
        await answerer.handleOffer('offer');
        expect(channel().sent).toEqual([]);
        channel().open();
        expect(channel().sent).toEqual(['{"screen":true}']);
    });

    test("the peer's screen shows only while its last state is true", async () => {
        const answerer = link(false);
        await answerer.handleOffer('offer');
        connection().fireTrack(0);
        connection().fireTrack(1);
        connection().fireTrack(2);
        expect(peerMedia.length).toBe(2);
        expect(peerScreens).toEqual([]);
        channel().receive('{"screen":true}');
        expect(peerScreens).toHaveLength(1);
        expect(peerScreens[0]).not.toBeNull();
        channel().receive('{"screen":false}');
        expect(peerScreens.at(-1)).toBeNull();
    });

    test('a state that arrives before the screen track waits for it', async () => {
        await link(false).handleOffer('offer');
        channel().receive('{"screen":true}');
        expect(peerScreens.at(-1) ?? null).toBeNull();
        connection().fireTrack(2);
        expect(peerScreens.at(-1)).not.toBeNull();
    });

    test('garbage on the channel changes nothing', async () => {
        await link(false).handleOffer('offer');
        connection().fireTrack(2);
        channel().receive('{"screen":true}');
        const shown = peerScreens.length;
        for (const hostile of ['nope', '{"screen":"yes"}', '[]', 7, new ArrayBuffer(2), '']) channel().receive(hostile);
        expect(peerScreens).toHaveLength(shown);
        expect(peerScreens.at(-1)).not.toBeNull();
    });

    test("the camera's media never carries the screen section's track", async () => {
        await link(false).handleOffer('offer');
        const camera = new Set<unknown>();
        connection().fireTrack(2);
        for (const stream of peerMedia) camera.add(stream);
        expect(camera.size).toBe(0);
    });

    test('stats are mapped by section order through the transceivers', async () => {
        const answerer = link(false);
        await answerer.handleOffer('offer');
        expect(answerer.sectionTrackIds()).toEqual(['remote-0', 'remote-1', 'remote-2']);
    });
});
