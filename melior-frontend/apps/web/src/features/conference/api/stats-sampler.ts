import {
    countInbound,
    type FlowState,
    flowingKinds,
    type InboundEntry,
    NO_OBSERVATION,
    type Path,
    type PathObservation,
    type SessionId,
    updateFlow,
} from '@/features/conference/domain';

export const SAMPLE_INTERVAL_MS = 250;

/** One connection whose inbound media is sampled. */
export type StatsSource = {
    connection: RTCPeerConnection;
    isConnected: () => boolean;
    /** Whose media an inbound stats entry shows; a direct link answers with its one peer for every entry. */
    peerOf: (entry: InboundEntry) => SessionId | undefined;
};

export type Observations = Map<SessionId, Record<Path, PathObservation>>;

type StatsSamplerOptions = {
    sfu: () => StatsSource | null;
    direct: () => StatsSource[];
    onSample: (observations: Observations) => void;
    now?: () => number;
};

const readInbound = async (connection: RTCPeerConnection): Promise<InboundEntry[]> => {
    const entries: InboundEntry[] = [];
    try {
        const report = await connection.getStats();
        report.forEach((entry) => {
            if (entry.type === 'inbound-rtp') entries.push(entry as InboundEntry);
        });
    } catch {
        // A closing connection has no stats; it simply shows nothing flowing.
    }
    return entries;
};

/**
 * Every 250 ms, reports for every peer which kinds flow on each path: SFU `inbound-rtp` entries are mapped to
 * peers through their receiving track, direct-link entries belong to the link's peer. A kind flows when its counter
 * grew within the last second.
 */
export const createStatsSampler = ({ sfu, direct, onSample, now = Date.now }: StatsSamplerOptions) => {
    const flows = { forwarded: new Map<SessionId, FlowState>(), direct: new Map<SessionId, FlowState>() };
    let busy = false;

    const sampleSource = async (path: Path, source: StatsSource, observed: Observations) => {
        const counters = countInbound(await readInbound(source.connection), source.peerOf);
        const at = now();
        const connected = source.isConnected();
        for (const [peer, count] of counters) {
            const state = updateFlow(flows[path].get(peer), count, at);
            flows[path].set(peer, state);
            const entry = observed.get(peer) ?? { forwarded: NO_OBSERVATION, direct: NO_OBSERVATION };
            entry[path] = { connected, flowing: flowingKinds(state, at) };
            observed.set(peer, entry);
        }
    };

    // A closed link must not leave counters behind: a new link to the same peer starts again from zero.
    const forgetUnobserved = (observed: Observations) => {
        for (const path of ['forwarded', 'direct'] as const) {
            for (const peer of [...flows[path].keys()]) {
                if (observed.get(peer)?.[path] === NO_OBSERVATION || !observed.has(peer)) flows[path].delete(peer);
            }
        }
    };

    const tick = async () => {
        if (busy) return;
        busy = true;
        try {
            const observed: Observations = new Map();
            const forwarded = sfu();
            const jobs = direct().map((source) => sampleSource('direct', source, observed));
            if (forwarded) jobs.push(sampleSource('forwarded', forwarded, observed));
            await Promise.all(jobs);
            forgetUnobserved(observed);
            onSample(observed);
        } finally {
            busy = false;
        }
    };

    const timer = setInterval(() => void tick(), SAMPLE_INTERVAL_MS);
    return { stop: () => clearInterval(timer) };
};
