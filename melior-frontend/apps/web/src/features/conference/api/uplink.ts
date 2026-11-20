export const UPLINK_RETRY_MS = 500;
export const UPLINK_MAX_RETRIES = 6;

/**
 * Sets `encodings[*].active` on every sender: pauses or resumes sending without renegotiating. A paused sender
 * keeps its track published; it only stops sending packets. Returns false when some sender could not be set
 * (its encodings do not exist yet, or `setParameters` failed), so the caller tries again. Stops early, reporting
 * success, once `isCurrent` says a newer apply took over.
 */
export const setSendersActive = async (
    connection: RTCPeerConnection,
    active: boolean,
    isCurrent: () => boolean = () => true,
): Promise<boolean> => {
    let complete = true;
    for (const sender of connection.getSenders()) {
        if (!isCurrent()) return true;
        if (!sender.track) continue;
        const parameters = sender.getParameters();
        if (!parameters.encodings?.length) {
            complete = false;
            continue;
        }
        for (const encoding of parameters.encodings) encoding.active = active;
        try {
            await sender.setParameters(parameters);
        } catch (error) {
            console.warn('setParameters failed', error);
            complete = false;
        }
    }
    return complete;
};

export type Uplink = {
    /** Records the wanted value and applies it. */
    set: (active: boolean) => Promise<void>;
    /** Applies the wanted value again, in both directions; a no-op until `set` was called once. */
    reapply: () => Promise<void>;
    stop: () => void;
};

/**
 * Keeps the wanted uplink state of one connection and retries until every sender has taken it. A room without
 * mesh never sends `uplink`, so nothing is ever touched there. Each apply gets a generation: a stale one
 * stops writing and does not schedule a retry, so overlapping applies cannot leave the old value behind.
 */
export const createUplink = (connection: RTCPeerConnection): Uplink => {
    let wanted: boolean | null = null;
    let generation = 0;
    let timer: ReturnType<typeof setTimeout> | null = null;

    const stop = () => {
        generation += 1;
        if (timer) clearTimeout(timer);
        timer = null;
    };

    const apply = async (retries: number): Promise<void> => {
        if (wanted === null) return;
        if (timer) clearTimeout(timer);
        timer = null;
        generation += 1;
        const mine = generation;
        const isCurrent = () => mine === generation;
        const complete = await setSendersActive(connection, wanted, isCurrent);
        if (complete || !isCurrent() || retries >= UPLINK_MAX_RETRIES) return;
        timer = setTimeout(() => void apply(retries + 1), UPLINK_RETRY_MS);
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
