import type { SessionId } from './session-id';
import type { TrackAssignment, TrackSource } from './signalling-message';

/** Structural stand-in for MediaStreamTrack so the domain stays free of DOM types. */
export type TrackRef = object;

/**
 * Names one stream of one participant. A participant's camera stream is named by its session id; its
 * screen is a stream of its own, so a viewer can show it large and the camera small.
 */
export type StreamKey = string;

const SCREEN_SUFFIX = '#screen';

export const streamKey = (sessionId: SessionId, source: TrackSource): StreamKey =>
    source === 'screen' ? `${sessionId}${SCREEN_SUFFIX}` : sessionId;

/** Splits a stream key back into the participant and what the stream shows. */
export const parseStreamKey = (key: StreamKey): { sessionId: SessionId; source: TrackSource } =>
    key.endsWith(SCREEN_SUFFIX)
        ? { sessionId: key.slice(0, -SCREEN_SUFFIX.length), source: 'screen' }
        : { sessionId: key, source: 'camera' };

/** Tracks per stream, in the order the server listed their media sections. */
export type TracksBySession<TTrack extends TrackRef> = Record<StreamKey, TTrack[]>;

/** Splits streams by what they show: cameras and screens, each by the participant's session id. */
export const partitionStreams = <TStream extends object>(
    streams: Record<StreamKey, TStream>,
): { camera: Record<SessionId, TStream>; screen: Record<SessionId, TStream> } => {
    const camera: Record<SessionId, TStream> = {};
    const screen: Record<SessionId, TStream> = {};
    for (const [key, stream] of Object.entries(streams)) {
        const { sessionId, source } = parseStreamKey(key);
        (source === 'screen' ? screen : camera)[sessionId] = stream;
    }
    return { camera, screen };
};

/**
 * Who publishes what: joins the server's `mid -> session_id` assignments with the receivers the peer
 * connection has (`mid -> track`). The assignments are the only source of truth: a section that is not
 * listed carries nothing, and a listed mid with no receiver (yet) is skipped. A mid listed twice counts
 * once, for its first participant.
 */
export const reconcileTracks = <TTrack extends TrackRef>(
    assignments: readonly TrackAssignment[],
    receivers: ReadonlyMap<string, TTrack>,
): TracksBySession<TTrack> => {
    const result: TracksBySession<TTrack> = {};
    const seen = new Set<string>();
    for (const { mid, session_id, source } of assignments) {
        if (seen.has(mid)) continue;
        seen.add(mid);
        const track = receivers.get(mid);
        if (!track) continue;
        (result[streamKey(session_id, source)] ??= []).push(track);
    }
    return result;
};

export type TrackChange<TTrack extends TrackRef> = { sessionId: StreamKey; track: TTrack };

export type TrackDiff<TTrack extends TrackRef> = {
    /** Remove these before adding anything: a reused section's track moves from one participant to another. */
    removed: TrackChange<TTrack>[];
    added: TrackChange<TTrack>[];
};

/** What changes between two reconciliations. Tracks are compared by identity. */
export const diffTracks = <TTrack extends TrackRef>(
    previous: TracksBySession<TTrack>,
    next: TracksBySession<TTrack>,
): TrackDiff<TTrack> => {
    const removed: TrackChange<TTrack>[] = [];
    const added: TrackChange<TTrack>[] = [];
    for (const [sessionId, tracks] of Object.entries(previous)) {
        for (const track of tracks) if (!next[sessionId]?.includes(track)) removed.push({ sessionId, track });
    }
    for (const [sessionId, tracks] of Object.entries(next)) {
        for (const track of tracks) if (!previous[sessionId]?.includes(track)) added.push({ sessionId, track });
    }
    return { removed, added };
};
