import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { createUplink, UPLINK_RETRY_MS } from './uplink';

type Params = { encodings: Array<{ active: boolean }>; token: number };

type SenderOptions = {
    encodings?: number;
    /** Plain failures before `setParameters` works. */
    failures?: number;
    /** `InvalidStateError`s before `setParameters` works. */
    staleFailures?: number;
    /** Hold every `setParameters` until the test settles it. */
    manual?: boolean;
};

/**
 * A sender that behaves like Chrome's: the object `getParameters()` returns is valid only until the current
 * task ends (a microtask queued at `getParameters` time clears it, so any `await` in between invalidates it),
 * and a finishing `setParameters` clears the sender's transaction, and a second one while one is pending fails.
 */
const makeSender = ({ encodings = 1, failures = 0, staleFailures = 0, manual = false }: SenderOptions = {}) => {
    let transaction: number | null = null;
    let inFlight = false;
    let counter = 0;
    let plain = failures;
    let stale = staleFailures;
    const sender = {
        track: {},
        active: Array.from({ length: encodings }, () => true),
        calls: [] as boolean[],
        held: [] as Array<() => void>,
        getParameters: (): Params => {
            counter += 1;
            const token = counter;
            transaction = token;
            queueMicrotask(() => {
                if (transaction === token) transaction = null;
            });
            return { encodings: sender.active.map((active) => ({ active })), token };
        },
        setParameters: async (parameters: Params): Promise<void> => {
            if (parameters.token !== transaction) {
                throw new DOMException('getParameters() has never been called on this sender', 'InvalidStateError');
            }
            if (inFlight) throw new DOMException('a setParameters is still pending', 'InvalidStateError');
            const value = parameters.encodings[0]?.active ?? true;
            sender.calls.push(value);
            inFlight = true;
            if (manual) await new Promise<void>((resolve) => sender.held.push(resolve));
            else await Promise.resolve();
            transaction = null;
            inFlight = false;
            if (stale > 0) {
                stale -= 1;
                throw new DOMException('stale', 'InvalidStateError');
            }
            if (plain > 0) {
                plain -= 1;
                throw new Error('not yet');
            }
            sender.active = sender.active.map(() => value);
        },
    };
    return sender;
};

const connectionWith = (...senders: Array<ReturnType<typeof makeSender>>) =>
    ({ getSenders: () => senders }) as unknown as RTCPeerConnection;

const flush = async () => {
    for (let i = 0; i < 10; i += 1) await Promise.resolve();
};

describe('createUplink', () => {
    let warn: ReturnType<typeof vi.spyOn>;

    beforeEach(() => {
        vi.useFakeTimers();
        warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    });

    afterEach(() => {
        vi.useRealTimers();
        vi.restoreAllMocks();
    });

    test('pauses and resumes every sender without warnings', async () => {
        const one = makeSender();
        const two = makeSender({ encodings: 2 });
        const uplink = createUplink(connectionWith(one, two));
        await uplink.set(false);
        expect(one.active).toEqual([false]);
        expect(two.active).toEqual([false, false]);
        await uplink.set(true);
        expect(one.active).toEqual([true]);
        expect(warn).not.toHaveBeenCalled();
    });

    test('retries while the encodings do not exist yet, then applies the wanted value', async () => {
        const early = makeSender({ encodings: 0 });
        const uplink = createUplink(connectionWith(early));
        await uplink.set(false);
        early.active = [true];
        await vi.advanceTimersByTimeAsync(UPLINK_RETRY_MS);
        expect(early.active).toEqual([false]);
        uplink.stop();
    });

    test('retries after a failed setParameters', async () => {
        const flaky = makeSender({ failures: 1 });
        const uplink = createUplink(connectionWith(flaky));
        await uplink.set(false);
        await vi.advanceTimersByTimeAsync(UPLINK_RETRY_MS);
        expect(flaky.active).toEqual([false]);
        uplink.stop();
    });

    test('an InvalidStateError is retried at once with fresh parameters, quietly', async () => {
        const stale = makeSender({ staleFailures: 1 });
        const uplink = createUplink(connectionWith(stale));
        await uplink.set(false);
        expect(stale.calls).toEqual([false, false]);
        expect(stale.active).toEqual([false]);
        expect(warn).not.toHaveBeenCalled();
    });

    test('reapply restores the wanted value after a negotiation reset it, in both directions', async () => {
        const one = makeSender();
        const uplink = createUplink(connectionWith(one));
        await uplink.set(false);
        one.active = [true];
        await uplink.reapply();
        expect(one.active).toEqual([false]);
        await uplink.set(true);
        one.active = [false];
        await uplink.reapply();
        expect(one.active).toEqual([true]);
    });

    test('reapply does nothing until a value was set (a room without mesh never touches the senders)', async () => {
        const one = makeSender();
        const uplink = createUplink(connectionWith(one));
        await uplink.reapply();
        expect(one.calls).toEqual([]);
    });

    test('overlapping applies run one after another: no stale parameters, the newer value wins', async () => {
        const first = makeSender({ manual: true });
        const second = makeSender({ manual: true });
        const uplink = createUplink(connectionWith(first, second));

        const paused = uplink.set(false);
        await flush();
        expect(first.calls).toEqual([false]);
        const resumed = uplink.set(true);
        first.held.shift()?.();
        await flush();
        // The older apply is stale and stops; the newer one starts on fresh parameters.
        expect(second.calls).toEqual([]);
        expect(first.calls).toEqual([false, true]);
        first.held.shift()?.();
        await flush();
        second.held.shift()?.();
        await Promise.all([paused, resumed]);

        expect(second.calls).toEqual([true]);
        expect(first.active).toEqual([true]);
        expect(second.active).toEqual([true]);
        expect(warn).not.toHaveBeenCalled();
    });
});
