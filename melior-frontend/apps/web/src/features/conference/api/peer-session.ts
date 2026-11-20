import {
    diffTracks,
    type IceCandidate,
    type IceServer,
    peerForTrack,
    reconcileTracks,
    type SessionId,
    type TrackAssignment,
    type TrackDiff,
    type TracksBySession,
} from '@/features/conference/domain';
import { createIceCandidateQueue } from './ice-candidate-queue';
import type { StatsSource } from './stats-sampler';
import { createUplink } from './uplink';

type PeerSessionOptions = {
    iceServers: IceServer[];
    /** Only use TURN relay candidates. For verifying the TURN path; needs a TURN server among `iceServers`. */
    forceRelay?: boolean;
    localStream: MediaStream | null;
    onIceCandidate: (candidate: IceCandidate) => void;
    /** The remote media after a renegotiation: one stable MediaStream per participant that has tracks. */
    onRemoteMedia: (streams: Record<SessionId, MediaStream>) => void;
    onConnected: () => void;
    onFailed: () => void;
};

export type PeerSession = {
    createOffer: () => Promise<string>;
    applyAnswer: (sdp: string) => Promise<void>;
    /** `tracks` is the server's complete, authoritative list of which media section carries whose track. */
    answerOffer: (sdp: string, tracks: TrackAssignment[]) => Promise<string>;
    addIceCandidate: (candidate: IceCandidate) => Promise<void>;
    /** Pauses or resumes sending on this connection (`uplink`), without renegotiation. */
    setUplinkActive: (active: boolean) => Promise<void>;
    /** What the stats sampler reads: this connection, with media sections mapped to their publisher. */
    statsSource: StatsSource;
    close: () => void;
};

/**
 * One MediaStream per participant, alive across renegotiations. A stream's id means nothing: a reused media
 * section keeps the msid of its first user, so who publishes a track comes from the server's
 * `mid -> session_id` list alone, never from `event.streams`.
 */
const createRemoteStreams = (onRemoteMedia: (streams: Record<SessionId, MediaStream>) => void) => {
    const streams = new Map<SessionId, MediaStream>();

    return {
        apply: ({ removed, added }: TrackDiff<MediaStreamTrack>, next: TracksBySession<MediaStreamTrack>) => {
            if (removed.length === 0 && added.length === 0) return;
            // Removals first: a reused section hands the same track object to another participant, and it
            // must leave the old participant's stream before it joins the new one.
            for (const { sessionId, track } of removed) streams.get(sessionId)?.removeTrack(track);
            for (const { sessionId, track } of added) {
                const stream = streams.get(sessionId) ?? new MediaStream();
                streams.set(sessionId, stream);
                stream.addTrack(track);
            }
            for (const sessionId of [...streams.keys()]) {
                if (!next[sessionId]) streams.delete(sessionId);
            }
            onRemoteMedia(Object.fromEntries(streams));
        },
    };
};

/** The SFU connection as the stats sampler reads it: an entry's receiving track leads to its publisher. */
const createSfuStatsSource = (connection: RTCPeerConnection, assignments: () => TrackAssignment[]): StatsSource => ({
    connection,
    isConnected: () => connection.connectionState === 'connected',
    peerOf: (entry) =>
        peerForTrack(
            entry.trackIdentifier,
            connection.getTransceivers().map((t) => ({ mid: t.mid, trackId: t.receiver.track.id })),
            assignments(),
        ),
});

export const createPeerSession = ({
    iceServers,
    forceRelay = false,
    localStream,
    onIceCandidate,
    onRemoteMedia,
    onConnected,
    onFailed,
}: PeerSessionOptions): PeerSession => {
    const connection = new RTCPeerConnection({ iceServers, iceTransportPolicy: forceRelay ? 'relay' : 'all' });
    const candidates = createIceCandidateQueue(connection, 'sfu');
    const uplink = createUplink(connection);
    let uplinkTimer: ReturnType<typeof setTimeout> | null = null;

    let assignments: TrackAssignment[] = [];
    let current: TracksBySession<MediaStreamTrack> = {};
    const remoteStreams = createRemoteStreams(onRemoteMedia);

    const reconcile = () => {
        const receivers = new Map<string, MediaStreamTrack>();
        for (const transceiver of connection.getTransceivers()) {
            if (transceiver.mid !== null) receivers.set(transceiver.mid, transceiver.receiver.track);
        }
        const next = reconcileTracks(assignments, receivers);
        const changes = diffTracks(current, next);
        current = next;
        remoteStreams.apply(changes, next);
    };

    connection.onicecandidate = (event) => {
        if (event.candidate) onIceCandidate(event.candidate.toJSON());
    };

    connection.onconnectionstatechange = () => {
        if (connection.connectionState === 'connected') onConnected();
        else if (connection.connectionState === 'failed') onFailed();
    };

    // After the negotiation's own messages went out: the caller sends the answer once we return.
    const reapplyUplinkSoon = () => {
        uplinkTimer = setTimeout(() => void uplink.reapply(), 0);
    };

    if (localStream) {
        for (const track of localStream.getTracks()) connection.addTrack(track, localStream);
    }

    return {
        statsSource: createSfuStatsSource(connection, () => assignments),
        setUplinkActive: uplink.set,
        createOffer: async () => {
            const offer = await connection.createOffer();
            await connection.setLocalDescription(offer);
            return connection.localDescription?.sdp ?? offer.sdp ?? '';
        },
        applyAnswer: async (sdp) => {
            await connection.setRemoteDescription({ type: 'answer', sdp });
            reconcile();
            await candidates.flush();
            reapplyUplinkSoon();
        },
        answerOffer: async (sdp, tracks) => {
            await connection.setRemoteDescription({ type: 'offer', sdp });
            // The transceivers and their mids exist now.
            assignments = tracks;
            reconcile();
            await candidates.flush();
            const answer = await connection.createAnswer();
            await connection.setLocalDescription(answer);
            reapplyUplinkSoon();
            return connection.localDescription?.sdp ?? answer.sdp ?? '';
        },
        addIceCandidate: candidates.add,
        close: () => {
            connection.onicecandidate = null;
            connection.onconnectionstatechange = null;
            if (uplinkTimer) clearTimeout(uplinkTimer);
            uplink.stop();
            connection.close();
        },
    };
};
