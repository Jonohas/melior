import type { SessionId } from './session-id';

export type MediaKind = 'audio' | 'video';

/** A kind is flowing when its counter grew within this many milliseconds. */
export const FLOW_WINDOW_MS = 1000;

/** The cumulative counters that show media arriving: video `framesDecoded`, audio `packetsReceived`. */
export type InboundCounters = Record<MediaKind, number>;

/** The fields of an `inbound-rtp` stats entry that matter here. */
export type InboundEntry = {
    kind: string;
    mid?: string | null;
    framesDecoded?: number;
    packetsReceived?: number;
};

/** Sums the counters per peer; `peerOfMid` says whose track a media section carries (undefined: nobody's). */
export const countInbound = (
    entries: readonly InboundEntry[],
    peerOfMid: (mid: string | null | undefined) => SessionId | undefined,
): Map<SessionId, InboundCounters> => {
    const result = new Map<SessionId, InboundCounters>();
    for (const entry of entries) {
        const peer = peerOfMid(entry.mid);
        if (peer === undefined) continue;
        const counters = result.get(peer) ?? { audio: 0, video: 0 };
        if (entry.kind === 'video') counters.video += entry.framesDecoded ?? 0;
        if (entry.kind === 'audio') counters.audio += entry.packetsReceived ?? 0;
        result.set(peer, counters);
    }
    return result;
};

export type FlowState = {
    last: InboundCounters;
    increasedAt: Record<MediaKind, number | null>;
};

const KINDS: readonly MediaKind[] = ['audio', 'video'];

/** Folds a new counter sample taken at `now` (ms) into the state. The first sample only sets the baseline. */
export const updateFlow = (previous: FlowState | undefined, counters: InboundCounters, now: number): FlowState => {
    const increasedAt = { audio: previous?.increasedAt.audio ?? null, video: previous?.increasedAt.video ?? null };
    if (previous) {
        for (const kind of KINDS) {
            if (counters[kind] > previous.last[kind]) increasedAt[kind] = now;
        }
    }
    return { last: { ...counters }, increasedAt };
};

export const flowingKinds = (state: FlowState | undefined, now: number): MediaKind[] =>
    KINDS.filter((kind) => {
        const at = state?.increasedAt[kind];
        return at !== null && at !== undefined && now - at <= FLOW_WINDOW_MS;
    });
