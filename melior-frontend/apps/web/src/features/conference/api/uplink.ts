export const UPLINK_RETRY_MS = 500;
export const UPLINK_MAX_RETRIES = 6;

/**
 * Sets `encodings[*].active` on every sender: pauses or resumes sending without renegotiating. A paused sender
 * keeps its track published; it only stops sending packets. Returns false when some sender could not be set
 * (its encodings do not exist yet, or `setParameters` failed), so the caller tries again.
 */
export const setSendersActive = async (connection: RTCPeerConnection, active: boolean): Promise<boolean> => {
    let complete = true;
    for (const sender of connection.getSenders()) {
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
    /** Applies the wanted value again, in both directions; call it after every negotiation. */
    reapply: () => Promise<void>;
    stop: () => void;
};

/** Keeps the wanted uplink state of one connection and retries until every sender has taken it. */
export const createUplink = (connection: RTCPeerConnection): Uplink => {
    let wanted = true;
    let timer: ReturnType<typeof setTimeout> | null = null;

    const stop = () => {
        if (timer) clearTimeout(timer);
        timer = null;
    };

    const apply = async (retries: number): Promise<void> => {
        stop();
        const complete = await setSendersActive(connection, wanted);
        if (complete || retries >= UPLINK_MAX_RETRIES) return;
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
