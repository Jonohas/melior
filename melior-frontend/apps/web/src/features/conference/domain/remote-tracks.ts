import type { SessionId } from './session-id';
import type { TrackAssignment } from './signalling-message';

/** Structural stand-in for MediaStreamTrack so the domain stays free of DOM types. */
export type TrackRef = object;

/** Tracks per participant, in the order the server listed their media sections. */
export type TracksBySession<TTrack extends TrackRef> = Record<SessionId, TTrack[]>;

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
    for (const { mid, session_id } of assignments) {
        if (seen.has(mid)) continue;
        seen.add(mid);
        const track = receivers.get(mid);
        if (!track) continue;
        (result[session_id] ??= []).push(track);
    }
    return result;
};

export type TrackChange<TTrack extends TrackRef> = { sessionId: SessionId; track: TTrack };

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
