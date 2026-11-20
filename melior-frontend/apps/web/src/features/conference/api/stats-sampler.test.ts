import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { FakePeerConnection, stubRtc } from './fake-rtc';
import { createStatsSampler, type Observations, SAMPLE_INTERVAL_MS } from './stats-sampler';

describe('createStatsSampler', () => {
    let clock: number;
    let samples: Observations[];
    let sfu: FakePeerConnection;
    let direct: FakePeerConnection;

    const sampler = () => {
        const asRtc = (connection: FakePeerConnection) => connection as unknown as RTCPeerConnection;
        return createStatsSampler({
            sfu: () => ({
                connection: asRtc(sfu),
                isConnected: () => true,
                sourceOf: (entry) => (entry.trackIdentifier === 'sfu-video' ? { peer: 'p', kind: 'video' } : undefined),
            }),
            direct: () => [
                {
                    connection: asRtc(direct),
                    isConnected: () => true,
                    sourceOf: (entry) => ({ peer: 'p', kind: entry.kind === 'audio' ? 'audio' : 'screen' }),
                },
            ],
            onSample: (observations) => samples.push(observations),
            now: () => clock,
        });
    };

    const tick = async () => {
        clock += SAMPLE_INTERVAL_MS;
        await vi.advanceTimersByTimeAsync(SAMPLE_INTERVAL_MS);
    };

    beforeEach(() => {
        vi.useFakeTimers();
        stubRtc();
        clock = 0;
        samples = [];
        sfu = new FakePeerConnection();
        direct = new FakePeerConnection();
    });

    afterEach(() => {
        vi.useRealTimers();
        vi.unstubAllGlobals();
    });

    test('reports which kinds flow per path, mapping SFU entries through their track and direct ones to the peer', async () => {
        sfu.statsEntries = [
            { type: 'inbound-rtp', kind: 'video', trackIdentifier: 'sfu-video', framesDecoded: 1 },
            { type: 'inbound-rtp', kind: 'video', trackIdentifier: 'unassigned', framesDecoded: 1 },
            { type: 'candidate-pair' },
        ];
        direct.statsEntries = [{ type: 'inbound-rtp', kind: 'audio', packetsReceived: 1 }];
        const running = sampler();
        await tick();
        expect(samples.at(-1)?.get('p')?.forwarded.flowing).toEqual([]);

        sfu.statsEntries[0] = { ...sfu.statsEntries[0], framesDecoded: 9 };
        direct.statsEntries = [{ type: 'inbound-rtp', kind: 'audio', packetsReceived: 5 }];
        await tick();
        const latest = samples.at(-1)?.get('p');
        expect(latest?.forwarded).toEqual({ connected: true, flowing: ['video'] });
        expect(latest?.direct).toEqual({ connected: true, flowing: ['audio'] });
        expect(samples.at(-1)?.size).toBe(1);
        running.stop();
    });

    test('a kind stops flowing a second after its counter stops growing', async () => {
        direct.statsEntries = [{ type: 'inbound-rtp', kind: 'audio', packetsReceived: 1 }];
        const running = sampler();
        await tick();
        direct.statsEntries = [{ type: 'inbound-rtp', kind: 'audio', packetsReceived: 2 }];
        await tick();
        expect(samples.at(-1)?.get('p')?.direct.flowing).toEqual(['audio']);
        for (let i = 0; i < 5; i += 1) await tick();
        expect(samples.at(-1)?.get('p')?.direct.flowing).toEqual([]);
        running.stop();
    });

    test('stop ends the sampling', async () => {
        const running = sampler();
        await tick();
        running.stop();
        const count = samples.length;
        await tick();
        expect(samples).toHaveLength(count);
    });
});
