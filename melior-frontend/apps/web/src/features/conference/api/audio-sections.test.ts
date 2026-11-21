import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { addAudioSection, protectAnsweredAudio } from './audio-sections';
import { FakePeerConnection, FakeTransceiver, stubRtc } from './fake-rtc';

const HIGH = { priority: 'high', networkPriority: 'high' };
const asConnection = (fake: FakePeerConnection) => fake as unknown as RTCPeerConnection;
const asTransceiver = (fake: FakeTransceiver) => fake as unknown as RTCRtpTransceiver;
const mimeTypes = (transceiver: FakeTransceiver) => transceiver.codecPreferences?.map((codec) => codec.mimeType);

describe('audio sections', () => {
    beforeEach(() => {
        stubRtc();
    });

    afterEach(() => {
        vi.unstubAllGlobals();
    });

    test('a section of our own sends at high priority and prefers RED', () => {
        const connection = new FakePeerConnection();
        addAudioSection(asConnection(connection), 'audio', { direction: 'sendrecv', streams: [] });
        const [section] = connection.transceivers;
        expect(section?.sendEncodings).toEqual([HIGH]);
        expect(section && mimeTypes(section)).toEqual(['audio/red', 'audio/opus', 'audio/PCMU']);
    });

    test('a browser without RED keeps its own codec order', () => {
        vi.stubGlobal('RTCRtpReceiver', { getCapabilities: () => ({ codecs: [{ mimeType: 'audio/opus' }] }) });
        const connection = new FakePeerConnection();
        addAudioSection(asConnection(connection), 'audio', { direction: 'sendrecv', streams: [] });
        expect(connection.transceivers[0]?.codecPreferences).toBeNull();
    });

    test('a browser without codec capabilities still gets its section', () => {
        vi.stubGlobal('RTCRtpReceiver', undefined);
        const connection = new FakePeerConnection();
        addAudioSection(asConnection(connection), 'audio', { direction: 'sendrecv', streams: [] });
        expect(connection.transceivers).toHaveLength(1);
    });

    test("a section the other side's offer created gets RED and the priority too", async () => {
        const section = new FakeTransceiver('audio', null, 'recvonly', 0);
        protectAnsweredAudio(asTransceiver(section));
        await Promise.resolve();
        expect(mimeTypes(section)?.[0]).toBe('audio/red');
        expect(section.sender.encodings).toEqual([{ active: true, ...HIGH }]);
    });

    test('a failing priority write only logs', async () => {
        const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
        const section = new FakeTransceiver('audio', null, 'recvonly', 0);
        section.sender.setParameters = async () => {
            throw new Error('refused');
        };
        protectAnsweredAudio(asTransceiver(section));
        await vi.waitFor(() => expect(warn).toHaveBeenCalled());
        warn.mockRestore();
    });
});
