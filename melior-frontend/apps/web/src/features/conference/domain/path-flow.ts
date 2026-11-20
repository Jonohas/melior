import type { SessionId } from './session-id';
import type { TrackAssignment } from './signalling-message';

/** What a peer sends: microphone, camera video, and shared screen video. */
export type MediaKind = 'audio' | 'video' | 'screen';

/** The kinds of the three media sections of a direct link, in section order. */
export const DIRECT_SECTIONS: readonly MediaKind[] = ['audio', 'video', 'screen'];

/** Whose media a stats entry shows, and which of the peer's kinds it is. */
export type InboundSource = { peer: SessionId; kind: MediaKind };

/** A kind is flowing when its counter grew within this many milliseconds. */
export const FLOW_WINDOW_MS = 1000;

/** The cumulative counters that show media arriving: `framesDecoded` for video and screen, `packetsReceived` for audio. */
export type InboundCounters = Record<MediaKind, number>;

/** The fields of an `inbound-rtp` stats entry that matter here. */
export type InboundEntry = {
    kind: string;
    /** The id of the receiving track; the stable link between a stats entry and a transceiver. */
    trackIdentifier?: string;
    framesDecoded?: number;
    packetsReceived?: number;
};

/** A media section of the SFU connection: its mid and the id of its receiving track. */
export type ReceiverSection = { mid: string | null; trackId: string };

const entryKind = (entry: InboundEntry): 'audio' | 'video' | undefined =>
    entry.kind === 'audio' || entry.kind === 'video' ? entry.kind : undefined;

/**
 * Who an SFU stats entry belongs to: the entry's `trackIdentifier` names a receiving track, the transceiver
 * holding it has a mid, and the server's assignments say who publishes that mid and whether it carries the
 * camera or the screen. Undefined when any link of that chain is missing.
 */
export const sfuSourceOf = (
    entry: InboundEntry,
    sections: readonly ReceiverSection[],
    assignments: readonly TrackAssignment[],
): InboundSource | undefined => {
    const mid = sections.find((section) => section.trackId === entry.trackIdentifier)?.mid;
    const kind = entryKind(entry);
    if (mid === null || mid === undefined || kind === undefined) return undefined;
    const assignment = assignments.find((candidate) => candidate.mid === mid);
    if (!assignment) return undefined;
    return { peer: assignment.session_id, kind: assignment.source === 'screen' ? 'screen' : kind };
};

/**
 * Which of its peer's kinds a direct-link stats entry shows: the position of its receiving track among the
 * link's sections (audio, camera, screen), given as the receiving track ids in section order.
 */
export const directKindOf = (entry: InboundEntry, trackIds: readonly string[]): MediaKind | undefined => {
    const index = trackIds.findIndex((id) => id === entry.trackIdentifier);
    return DIRECT_SECTIONS[index];
};

/** Sums the counters per peer; `sourceOf` says whose media an entry shows (undefined: nobody's). */
export const countInbound = (
    entries: readonly InboundEntry[],
    sourceOf: (entry: InboundEntry) => InboundSource | undefined,
): Map<SessionId, InboundCounters> => {
    const result = new Map<SessionId, InboundCounters>();
    for (const entry of entries) {
        const source = sourceOf(entry);
        if (source === undefined) continue;
        const counters = result.get(source.peer) ?? { audio: 0, video: 0, screen: 0 };
        counters[source.kind] += (source.kind === 'audio' ? entry.packetsReceived : entry.framesDecoded) ?? 0;
        result.set(source.peer, counters);
    }
    return result;
};

export type FlowState = {
    last: InboundCounters;
    increasedAt: Record<MediaKind, number | null>;
};

const KINDS = DIRECT_SECTIONS;

/** Folds a new counter sample taken at `now` (ms) into the state. The first sample only sets the baseline. */
export const updateFlow = (previous: FlowState | undefined, counters: InboundCounters, now: number): FlowState => {
    const increasedAt = {
        audio: previous?.increasedAt.audio ?? null,
        video: previous?.increasedAt.video ?? null,
        screen: previous?.increasedAt.screen ?? null,
    };
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
