import { applySenderPolicy, type SenderPolicy, type SendParametersLike } from '@/features/conference/domain';

export const UPLINK_RETRY_MS = 500;
export const UPLINK_MAX_RETRIES = 6;

/** The compression settings of one sender, or null to leave its settings alone. */
export type PolicyOf = (sender: RTCRtpSender) => SenderPolicy | null;

const NO_POLICY: PolicyOf = () => null;

const isInvalidState = (error: unknown): boolean => error instanceof DOMException && error.name === 'InvalidStateError';

/**
 * `getParameters()` → modify → `setParameters()` with nothing awaited in between: the browser only honours the
 * parameters object until the current task ends, and a `setParameters` still running on the same sender clears
 * it. Retries once at once with fresh parameters when the browser still calls it stale. `active` null leaves
 * the encodings' `active` alone and writes only the policy.
 */
const writeSender = async (
    sender: RTCRtpSender,
    active: boolean | null,
    policy: SenderPolicy | null,
    retry = true,
): Promise<boolean> => {
    const parameters = sender.getParameters();
    if (!parameters.encodings?.length) return false;
    if (!applySenderPolicy(parameters as unknown as SendParametersLike, policy, active)) return true;
    try {
        await sender.setParameters(parameters);
        return true;
    } catch (error) {
        if (retry && isInvalidState(error)) return writeSender(sender, active, policy, false);
        console.warn('setParameters failed', error);
        return false;
    }
};

/**
 * Writes `encodings[*].active` and each sender's compression policy on every sender with a track: pauses or
 * resumes sending without renegotiating, and keeps the settings a negotiation may have reset. A paused sender
 * keeps its track published; it only stops sending packets. Returns false when some sender could not be set
 * (its encodings do not exist yet, or `setParameters` failed), so the caller tries again. Stops early, reporting
 * success, once `isCurrent` says a newer apply took over.
 */
export const setSendersActive = async (
    connection: RTCPeerConnection,
    active: boolean | null,
    isCurrent: () => boolean = () => true,
    policyOf: PolicyOf = NO_POLICY,
): Promise<boolean> => {
    let complete = true;
    for (const sender of connection.getSenders()) {
        if (!isCurrent()) return true;
        if (!sender.track) continue;
        if (!(await writeSender(sender, active, policyOf(sender)))) complete = false;
    }
    return complete;
};

export type Uplink = {
    /** Records the wanted value and applies it. */
    set: (active: boolean) => Promise<void>;
    /**
     * Applies the wanted value and the policy again, after a negotiation that may have reset them. Without a
     * policy, a no-op until `set` was called once.
     */
    reapply: () => Promise<void>;
    stop: () => void;
};

/**
 * The one writer of a connection's sender parameters: the wanted uplink state and each sender's compression
 * policy go out together, because a second `setParameters` on a sender fails while one runs. Retries until every
 * sender has taken them. A room without mesh never sends `uplink`, so `active` is never touched there; without
 * a policy either, nothing is. Each apply gets a generation: a stale one
 * stops writing and does not schedule a retry, so overlapping applies cannot leave the old value behind.
 */
export const createUplink = (connection: RTCPeerConnection, policyOf: PolicyOf = NO_POLICY): Uplink => {
    let wanted: boolean | null = null;
    let generation = 0;
    let timer: ReturnType<typeof setTimeout> | null = null;
    // Applies run one after another: two in flight on one sender would clear each other's parameters.
    let queue: Promise<void> = Promise.resolve();

    const stop = () => {
        generation += 1;
        if (timer) clearTimeout(timer);
        timer = null;
    };

    const apply = (retries: number): Promise<void> => {
        if (wanted === null && policyOf === NO_POLICY) return Promise.resolve();
        if (timer) clearTimeout(timer);
        timer = null;
        generation += 1;
        const mine = generation;
        const isCurrent = () => mine === generation;
        const run = queue.then(async () => {
            if (!isCurrent()) return;
            const complete = await setSendersActive(connection, wanted, isCurrent, policyOf);
            if (complete || !isCurrent() || retries >= UPLINK_MAX_RETRIES) return;
            timer = setTimeout(() => void apply(retries + 1), UPLINK_RETRY_MS);
        });
        queue = run.catch(() => undefined);
        return run;
    };

    return {
        set: (active) => {
            wanted = active;
            return apply(0);
        },
        reapply: () => apply(0),
        stop,
    };
};
