import {
    diffTracks,
    type IceCandidate,
    type IceServer,
    reconcileTracks,
    type SessionId,
    type TrackAssignment,
    type TracksBySession,
} from '@/features/conference/domain';
import type { StatsSource } from './stats-sampler';
import { setSendersActive } from './uplink';

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
    let candidateQueue: IceCandidate[] = [];

    // One MediaStream per participant, alive across renegotiations. A stream's id means nothing: a reused
    // media section keeps the msid of its first user, so who publishes a track comes from the server's
    // `mid -> session_id` list alone, never from `event.streams`.
    const streams = new Map<SessionId, MediaStream>();
    let assignments: TrackAssignment[] = [];
    let current: TracksBySession<MediaStreamTrack> = {};
    let uplinkActive = true;

    const reconcile = () => {
        const receivers = new Map<string, MediaStreamTrack>();
        for (const transceiver of connection.getTransceivers()) {
            if (transceiver.mid !== null) receivers.set(transceiver.mid, transceiver.receiver.track);
        }
        const next = reconcileTracks(assignments, receivers);
        const { removed, added } = diffTracks(current, next);
        current = next;
        if (removed.length === 0 && added.length === 0) return;

        // Removals first: a reused section hands the same track object to another participant, and it must
        // leave the old participant's stream before it joins the new one.
        for (const { sessionId, track } of removed) streams.get(sessionId)?.removeTrack(track);
        for (const { sessionId, track } of added) {
            let stream = streams.get(sessionId);
            if (!stream) {
                stream = new MediaStream();
                streams.set(sessionId, stream);
            }
            stream.addTrack(track);
        }
        for (const sessionId of [...streams.keys()]) {
            if (!next[sessionId]) streams.delete(sessionId);
        }
        onRemoteMedia(Object.fromEntries(streams));
    };

    connection.onicecandidate = (event) => {
        if (event.candidate) onIceCandidate(event.candidate.toJSON());
    };

    connection.onconnectionstatechange = () => {
        if (connection.connectionState === 'connected') onConnected();
        else if (connection.connectionState === 'failed') onFailed();
    };

    if (localStream) {
        for (const track of localStream.getTracks()) connection.addTrack(track, localStream);
    }

    const flushCandidates = async () => {
        const queued = candidateQueue;
        candidateQueue = [];
        for (const candidate of queued) {
            try {
                await connection.addIceCandidate(candidate);
            } catch (error) {
                console.warn('addIceCandidate (queued) failed', error);
            }
        }
    };

    // A negotiation may add senders; a paused uplink stays paused.
    const reapplyUplink = async () => {
        if (!uplinkActive) await setSendersActive(connection, false);
    };

    return {
        statsSource: {
            connection,
            isConnected: () => connection.connectionState === 'connected',
            peerOfMid: (mid) => assignments.find((a) => a.mid === mid)?.session_id,
        },
        setUplinkActive: async (active) => {
            uplinkActive = active;
            await setSendersActive(connection, active);
        },
        createOffer: async () => {
            const offer = await connection.createOffer();
            await connection.setLocalDescription(offer);
            return connection.localDescription?.sdp ?? offer.sdp ?? '';
        },
        applyAnswer: async (sdp) => {
            await connection.setRemoteDescription({ type: 'answer', sdp });
            reconcile();
            await flushCandidates();
            await reapplyUplink();
        },
        answerOffer: async (sdp, tracks) => {
            await connection.setRemoteDescription({ type: 'offer', sdp });
            // The transceivers and their mids exist now.
            assignments = tracks;
            reconcile();
            await flushCandidates();
            const answer = await connection.createAnswer();
            await connection.setLocalDescription(answer);
            await reapplyUplink();
            return connection.localDescription?.sdp ?? answer.sdp ?? '';
        },
        addIceCandidate: async (candidate) => {
            if (!connection.remoteDescription) {
                candidateQueue.push(candidate);
                return;
            }
            try {
                await connection.addIceCandidate(candidate);
            } catch (error) {
                console.warn('addIceCandidate failed', error);
            }
        },
        close: () => {
            connection.onicecandidate = null;
            connection.onconnectionstatechange = null;
            connection.close();
        },
    };
};
