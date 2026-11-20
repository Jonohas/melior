import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { FakeMediaStream, FakePeerConnection, stubRtc } from './fake-rtc';
import { createLinkRegistry, type LinkRegistry } from './link-registry';

const track = (id: string, kind: string) => ({ id, kind, enabled: true });
const asTrack = (value: { id: string }) => value as unknown as MediaStreamTrack;
const MIC = track('mic', 'audio');
const CAMERA = track('cam', 'video');
const SCREEN = track('scr', 'video');
const NEW_MIC = track('mic2', 'audio');
const NEW_CAMERA = track('cam2', 'video');

// 'a' sorts below 'b' and 'c', so this side offers to both.
describe('createLinkRegistry', () => {
    let registry: LinkRegistry;

    const create = (screenTrack: MediaStreamTrack | null = null) =>
        createLinkRegistry({
            selfId: 'a',
            iceServers: [],
            forceRelay: false,
            localStream: new FakeMediaStream([MIC, CAMERA]) as unknown as MediaStream,
            screenTrack,
            send: () => undefined,
            onMedia: () => undefined,
            onScreen: () => undefined,
            onFailed: () => undefined,
        });

    const sections = (index: number) => FakePeerConnection.instances[index]?.transceivers ?? [];
    const tracksOf = (index: number) => sections(index).map((t) => t.sender.track?.id ?? null);

    beforeEach(() => {
        stubRtc();
    });

    afterEach(() => {
        registry.closeAll();
        vi.unstubAllGlobals();
    });

    test('starting a share replaces the screen track on every open link, and a link opened later starts with it', async () => {
        registry = create();
        await registry.open('b').start();
        await registry.open('c').start();
        await registry.setScreen(asTrack(SCREEN));
        expect(tracksOf(0)).toEqual(['mic', 'cam', 'scr']);
        expect(tracksOf(1)).toEqual(['mic', 'cam', 'scr']);
        await registry.open('d').start();
        expect(tracksOf(2)).toEqual(['mic', 'cam', 'scr']);
    });

    test('stopping a share empties the screen section of every link; none is renegotiated', async () => {
        registry = create(asTrack(SCREEN));
        await registry.open('b').start();
        expect(tracksOf(0)).toEqual(['mic', 'cam', 'scr']);
        await registry.setScreen(null);
        expect(tracksOf(0)).toEqual(['mic', 'cam', null]);
        expect(FakePeerConnection.instances[0]?.offersCreated).toBe(1);
        await registry.open('c').start();
        expect(tracksOf(1)).toEqual(['mic', 'cam', null]);
    });

    test('a device switch replaces the matching track on every open link before anything is stopped', async () => {
        registry = create();
        await registry.open('b').start();
        await registry.open('c').start();
        await registry.replaceInput(asTrack(NEW_CAMERA));
        await registry.replaceInput(asTrack(NEW_MIC));
        expect(tracksOf(0)).toEqual(['mic2', 'cam2', null]);
        expect(tracksOf(1)).toEqual(['mic2', 'cam2', null]);
        expect(FakePeerConnection.instances.every((c) => c.offersCreated === 1)).toBe(true);
    });

    test('a replaced track keeps the mute state it was given, and so does one on a later link', async () => {
        registry = create();
        await registry.open('b').start();
        const muted = { ...NEW_MIC, enabled: false };
        await registry.replaceInput(asTrack(muted));
        expect(sections(0)[0]?.sender.track?.enabled).toBe(false);
        await registry.open('c').start();
        expect(sections(1)[0]?.sender.track?.enabled).toBe(false);
    });

    test('a link opened after a switch uses the current tracks', async () => {
        registry = create();
        await registry.replaceInput(asTrack(NEW_CAMERA));
        await registry.open('b').start();
        expect(tracksOf(0)).toEqual(['mic', 'cam2', null]);
    });

    test('maps direct-link stats entries to the peer and the kind of their section', async () => {
        registry = create();
        await registry.open('b').start();
        const [source] = registry.statsSources();
        const kinds = ['remote-0', 'remote-1', 'remote-2'].map(
            (id) => source?.sourceOf({ kind: id === 'remote-0' ? 'audio' : 'video', trackIdentifier: id })?.kind,
        );
        expect(kinds).toEqual(['audio', 'video', 'screen']);
        expect(source?.sourceOf({ kind: 'video', trackIdentifier: 'x' })).toBeUndefined();
        expect(source?.sourceOf({ kind: 'video', trackIdentifier: 'remote-2' })?.peer).toBe('b');
    });

    test('one link that cannot take a new track does not hurt the others or reject', async () => {
        const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
        registry = create();
        await registry.open('b').start();
        await registry.open('c').start();
        const failing = sections(0)[1]?.sender;
        if (!failing) throw new Error('no camera section');
        failing.failing = true;
        await expect(registry.replaceInput(asTrack(NEW_CAMERA))).resolves.toBeUndefined();
        expect(tracksOf(0)).toEqual(['mic', 'cam', null]);
        expect(tracksOf(1)).toEqual(['mic', 'cam2', null]);
        expect(warn).toHaveBeenCalledTimes(1);
        await registry.open('d').start();
        expect(tracksOf(2)).toEqual(['mic', 'cam2', null]);
        warn.mockRestore();
    });

    test('one link that cannot take the screen does not stop the others, on start and on stop', async () => {
        const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
        registry = create();
        await registry.open('b').start();
        await registry.open('c').start();
        const failing = sections(0)[2]?.sender;
        if (!failing) throw new Error('no screen section');
        failing.failing = true;
        await expect(registry.setScreen(asTrack(SCREEN))).resolves.toBeUndefined();
        expect(tracksOf(1)).toEqual(['mic', 'cam', 'scr']);
        await expect(registry.setScreen(null)).resolves.toBeUndefined();
        expect(tracksOf(1)).toEqual(['mic', 'cam', null]);
        expect(warn).toHaveBeenCalledTimes(2);
        warn.mockRestore();
    });
});
