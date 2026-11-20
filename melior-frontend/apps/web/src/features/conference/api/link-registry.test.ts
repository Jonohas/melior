import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { FakeMediaStream, FakePeerConnection, stubRtc } from './fake-rtc';
import { createLinkRegistry, type LinkRegistry } from './link-registry';

const track = (id: string, kind: string) => ({ id, kind, enabled: true });
const asTrack = (value: { id: string }) => value as unknown as MediaStreamTrack;
const MIC = track('mic', 'audio');
const CAMERA = track('cam', 'video');
const SCREEN = track('scr', 'video');

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
});
