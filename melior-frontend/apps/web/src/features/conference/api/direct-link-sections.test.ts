import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { createDirectLink, type DirectLink } from './direct-link';
import type { LocalTracks } from './direct-sections';
import { FakeMediaStream, FakePeerConnection, FakeTransceiver, stubRtc } from './fake-rtc';

const track = (id: string, kind: string) => ({ id, kind }) as unknown as MediaStreamTrack;
const MIC = track('mic', 'audio');
const CAMERA = track('cam', 'video');
const SCREEN = track('scr', 'video');
const SCREEN_AUDIO = track('scra', 'audio');
const SHARED = { screen: SCREEN, screenAudio: SCREEN_AUDIO };
const NOT_SHARED = { screen: null, screenAudio: null };

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
        local = { audio: MIC, video: CAMERA, ...NOT_SHARED };
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

    test('the offerer has audio, camera, screen and screen audio sections in that order, and negotiates once', async () => {
        const offerer = link(true);
        await offerer.start();
        await offerer.start();
        expect(sections().map((t) => [t.receiver.track.kind, t.sender.track?.id ?? null, t.direction])).toEqual([
            ['audio', 'mic', 'sendrecv'],
            ['video', 'cam', 'sendrecv'],
            ['video', null, 'sendrecv'],
            ['audio', null, 'sendrecv'],
        ]);
        expect(connection().offersCreated).toBe(1);
    });

    test('an offerer without a camera still has all four sections', async () => {
        local = { audio: MIC, video: null, ...NOT_SHARED };
        await link(true).start();
        expect(sections().map((t) => t.receiver.track.kind)).toEqual(['audio', 'video', 'video', 'audio']);
    });

    test('a link opened during a share starts with the screen and its sound on their sections', async () => {
        local = { ...local, ...SHARED };
        await link(true).start();
        expect(sections()[2]?.sender.track).toBe(SCREEN);
        expect(sections()[3]?.sender.track).toBe(SCREEN_AUDIO);
    });

    test('the answerer attaches its tracks to the offer sections and sends on all four', async () => {
        local = { ...local, ...SHARED };
        const answerer = link(false);
        await answerer.handleOffer('offer');
        expect(sections().map((t) => [t.sender.track?.id, t.direction])).toEqual([
            ['mic', 'sendrecv'],
            ['cam', 'sendrecv'],
            ['scr', 'sendrecv'],
            ['scra', 'sendrecv'],
        ]);
        expect(connection().tracksAdded).toBe(0);
    });

    test('the answerer without a screen leaves its screen section empty, still sending direction', async () => {
        await link(false).handleOffer('offer');
        expect(sections()[2]?.sender.track).toBeNull();
        expect(sections()[2]?.direction).toBe('sendrecv');
    });

    test("starting and stopping a share replaces the screen's tracks and tells the peer, without renegotiating", async () => {
        const offerer = link(true);
        await offerer.start();
        channel().open();
        expect(channel().sent).toEqual(['{"screen":false}']);
        local = { ...local, ...SHARED };
        await offerer.setScreen(SHARED);
        expect(sections()[2]?.sender.track).toBe(SCREEN);
        expect(sections()[3]?.sender.track).toBe(SCREEN_AUDIO);
        local = { ...local, ...NOT_SHARED };
        await offerer.setScreen(NOT_SHARED);
        expect(sections()[2]?.sender.track).toBeNull();
        expect(sections()[3]?.sender.track).toBeNull();
        expect(channel().sent).toEqual(['{"screen":false}', '{"screen":true}', '{"screen":false}']);
        expect(connection().offersCreated).toBe(1);
    });

    test('a sharer announces itself when the channel opens', async () => {
        local = { ...local, ...SHARED };
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

    test("the peer's screen carries its sound, never the camera's media", async () => {
        await link(false).handleOffer('offer');
        connection().fireTrack(2);
        connection().fireTrack(3);
        channel().receive('{"screen":true}');
        const screen = peerScreens.at(-1) as FakeMediaStream;
        expect(screen.getTracks().map((t) => t.id)).toEqual(['remote-2', 'remote-3']);
        expect(peerMedia).toEqual([]);
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
        expect(answerer.sectionTrackIds()).toEqual(['remote-0', 'remote-1', 'remote-2', 'remote-3']);
    });

    test('a screen change while the answerer attaches its sections is not lost', async () => {
        const answerer = link(false);
        const newCamera = track('cam2', 'video');
        connection().afterSetRemoteOffer = () => {
            const audio = sections()[0];
            if (!audio) throw new Error('no audio section');
            // The device is switched and a share starts while the first track is being attached.
            audio.sender.duringReplace = () => {
                local = { audio: MIC, video: newCamera, ...SHARED };
            };
        };
        await answerer.handleOffer('offer');
        expect(sections().map((t) => t.sender.track?.id ?? null)).toEqual(['mic', 'cam2', 'scr', 'scra']);
    });

    test('a failing screen section still tells the peer the new state, on start and on stop', async () => {
        const offerer = link(true);
        await offerer.start();
        channel().open();
        const screenSender = sections()[2]?.sender;
        if (!screenSender) throw new Error('no screen section');
        screenSender.failing = true;
        local = { ...local, ...SHARED };
        await expect(offerer.setScreen(SHARED)).rejects.toThrow('replaceTrack failed');
        local = { ...local, ...NOT_SHARED };
        await expect(offerer.setScreen(NOT_SHARED)).rejects.toThrow('replaceTrack failed');
        expect(channel().sent).toEqual(['{"screen":false}', '{"screen":true}', '{"screen":false}']);
    });
});
