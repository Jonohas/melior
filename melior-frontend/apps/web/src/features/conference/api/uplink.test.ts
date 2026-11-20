import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { createUplink, UPLINK_RETRY_MS } from './uplink';

type FakeSender = {
    track: object;
    encodings: Array<{ active: boolean }>;
    failures: number;
    getParameters: () => { encodings: Array<{ active: boolean }> };
    setParameters: (parameters: { encodings: Array<{ active: boolean }> }) => Promise<void>;
};

const sender = (encodingCount: number, failures = 0): FakeSender => {
    const fake: FakeSender = {
        track: {},
        encodings: Array.from({ length: encodingCount }, () => ({ active: true })),
        failures,
        getParameters: () => ({ encodings: fake.encodings }),
        setParameters: async () => {
            if (fake.failures > 0) {
                fake.failures -= 1;
                throw new Error('not yet');
            }
        },
    };
    return fake;
};

const connectionWith = (...senders: FakeSender[]) => ({ getSenders: () => senders }) as unknown as RTCPeerConnection;

describe('createUplink', () => {
    beforeEach(() => {
        vi.useFakeTimers();
        vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    });

    afterEach(() => {
        vi.useRealTimers();
        vi.restoreAllMocks();
    });

    test('pauses and resumes every sender', async () => {
        const one = sender(1);
        const uplink = createUplink(connectionWith(one, sender(2)));
        await uplink.set(false);
        expect(one.encodings[0]?.active).toBe(false);
        await uplink.set(true);
        expect(one.encodings[0]?.active).toBe(true);
    });

    test('retries while the encodings do not exist yet, then applies the wanted value', async () => {
        const early = sender(0);
        const uplink = createUplink(connectionWith(early));
        await uplink.set(false);
        early.encodings.push({ active: true });
        await vi.advanceTimersByTimeAsync(UPLINK_RETRY_MS);
        expect(early.encodings[0]?.active).toBe(false);
        uplink.stop();
    });

    test('retries after a failed setParameters', async () => {
        const flaky = sender(1, 1);
        const uplink = createUplink(connectionWith(flaky));
        await uplink.set(false);
        await vi.advanceTimersByTimeAsync(UPLINK_RETRY_MS);
        expect(flaky.failures).toBe(0);
        expect(flaky.encodings[0]?.active).toBe(false);
        uplink.stop();
    });

    test('reapply restores the wanted value after a negotiation reset it, in both directions', async () => {
        const one = sender(1);
        const uplink = createUplink(connectionWith(one));
        await uplink.set(false);
        one.encodings[0] = { active: true };
        await uplink.reapply();
        expect(one.encodings[0]?.active).toBe(false);
        await uplink.set(true);
        one.encodings[0] = { active: false };
        await uplink.reapply();
        expect(one.encodings[0]?.active).toBe(true);
    });
});

type Pending = { value: boolean; settle: () => void };

/** A sender whose `setParameters` calls stay pending until the test settles them; a settled call commits. */
const manualSender = (pending: Pending[], committed: { value: boolean | null }) => ({
    track: {},
    getParameters: () => ({ encodings: [{ active: true }] }),
    setParameters: (parameters: { encodings: Array<{ active: boolean }> }) =>
        new Promise<void>((resolve) => {
            const value = parameters.encodings[0]?.active ?? true;
            pending.push({
                value,
                settle: () => {
                    committed.value = value;
                    resolve();
                },
            });
        }),
});

describe('createUplink ordering', () => {
    test('reapply does nothing until a value was set (a room without mesh never touches the senders)', async () => {
        const setParameters = vi.fn();
        const connection = {
            getSenders: () => [{ track: {}, getParameters: () => ({ encodings: [{ active: true }] }), setParameters }],
        } as unknown as RTCPeerConnection;
        const uplink = createUplink(connection);
        await uplink.reapply();
        expect(setParameters).not.toHaveBeenCalled();
    });

    test('an older apply that finishes late does not write over a newer one', async () => {
        const pending: Pending[][] = [[], []];
        const committed = [{ value: null as boolean | null }, { value: null as boolean | null }];
        const first = manualSender(pending[0] ?? [], committed[0] ?? { value: null });
        const second = manualSender(pending[1] ?? [], committed[1] ?? { value: null });
        const connection = { getSenders: () => [first, second] } as unknown as RTCPeerConnection;
        const uplink = createUplink(connection);

        const paused = uplink.set(false);
        const resumed = uplink.set(true);
        // The newer apply gets through the first sender and reaches the second before the older one is done.
        pending[0]?.[1]?.settle();
        await Promise.resolve();
        await Promise.resolve();
        pending[1]?.[0]?.settle();
        pending[0]?.[0]?.settle();
        await Promise.resolve();
        await Promise.resolve();
        pending[1]?.at(-1)?.settle();
        await Promise.all([paused, resumed]);

        expect(committed[1]?.value).toBe(true);
        expect(pending[1]?.map((call) => call.value)).toEqual([true]);
    });
});
