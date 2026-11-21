import {
    diffTracks,
    type IceCandidate,
    type IceServer,
    reconcileTracks,
    type StreamKey,
    sfuSourceOf,
    type TrackAssignment,
    type TrackDiff,
    type TracksBySession,
} from '@/features/conference/domain';
import { addAudioSection, type SectionInit } from './audio-sections';
import { createIceCandidateQueue } from './ice-candidate-queue';
import type { StatsSource } from './stats-sampler';
import { createUplink } from './uplink';

type PeerSessionOptions = {
    iceServers: IceServer[];
    /** Only use TURN relay candidates. For verifying the TURN path; needs a TURN server among `iceServers`. */
    forceRelay?: boolean;
    localStream: MediaStream | null;
    onIceCandidate: (candidate: IceCandidate) => void;
    /**
     * An offer for a change this side made after the connection was up (a screen added or removed). The
     * first offer is not one of these: the caller asks for it with `createOffer`.
     */
    onOffer: (sdp: string) => void;
    /**
     * The remote media after a renegotiation: one stable MediaStream per participant's camera and per
     * shared screen, named by `streamKey`.
     */
    onRemoteMedia: (streams: Record<StreamKey, MediaStream>) => void;
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
    /** Sends another microphone or camera from now on, in place of the one in use; no renegotiation. */
    replaceInput: (track: MediaStreamTrack) => Promise<void>;
    /** Starts sending a captured screen, and its sound if it has one, next to the camera. The change is offered to the server. */
    shareScreen: (screen: MediaStream) => void;
    /** Stops sending the screen. The change is offered to the server. */
    stopScreen: () => void;
    close: () => void;
};

/**
 * One MediaStream per participant, alive across renegotiations. A stream's id means nothing: a reused media
 * section keeps the msid of its first user, so who publishes a track comes from the server's
 * `mid -> session_id` list alone, never from `event.streams`.
 */
const createRemoteStreams = (onRemoteMedia: (streams: Record<StreamKey, MediaStream>) => void) => {
    const streams = new Map<StreamKey, MediaStream>();

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
    sourceOf: (entry) =>
        sfuSourceOf(
            entry,
            connection.getTransceivers().map((t) => ({ mid: t.mid, trackId: t.receiver.track.id })),
            assignments(),
        ),
});

export const createPeerSession = ({
    iceServers,
    forceRelay = false,
    localStream,
    onIceCandidate,
    onOffer,
    onRemoteMedia,
    onConnected,
    onFailed,
}: PeerSessionOptions): PeerSession => {
    const connection = new RTCPeerConnection({ iceServers, iceTransportPolicy: forceRelay ? 'relay' : 'all' });
    const candidates = createIceCandidateQueue(connection, 'sfu');
    const uplink = createUplink(connection);
    let uplinkTimer: ReturnType<typeof setTimeout> | null = null;

    let assignments: TrackAssignment[] = [];

    // Offers this side starts after the first exchange. Only one is in flight; one that crosses the
    // server's own offer is given up (the server ignores it) and made again once the connection is stable.
    let firstAnswerApplied = false;
    let offerInFlight = false;
    let changeWaiting = false;
    // The screen's picture and, when it was captured with sound, its audio.
    let screenSenders: RTCRtpSender[] = [];
    let current: TracksBySession<MediaStreamTrack> = {};
    const remoteStreams = createRemoteStreams(onRemoteMedia);

    /**
     * Asks the browser to show media as soon as it arrives, with no extra buffering. A call is
     * conversation, so a late frame is worth less than a glitch. Browsers that do not know these
     * properties ignore them.
     */
    const playLive = (receiver: RTCRtpReceiver) => {
        const live = receiver as RTCRtpReceiver & {
            jitterBufferTarget?: number | null;
            playoutDelayHint?: number | null;
        };
        try {
            live.jitterBufferTarget = 0;
            live.playoutDelayHint = 0;
        } catch {
            // Not supported here.
        }
    };

    const reconcile = () => {
        const receivers = new Map<string, MediaStreamTrack>();
        for (const transceiver of connection.getTransceivers()) {
            if (transceiver.mid === null) continue;
            playLive(transceiver.receiver);
            receivers.set(transceiver.mid, transceiver.receiver.track);
        }
        const next = reconcileTracks(assignments, receivers);
        const changes = diffTracks(current, next);
        current = next;
        remoteStreams.apply(changes, next);
    };

    connection.onicecandidate = (event) => {
        if (event.candidate) onIceCandidate(event.candidate.toJSON());
    };

    const offerWaitingChange = async () => {
        if (!firstAnswerApplied || offerInFlight || !changeWaiting) return;
        if (connection.signalingState !== 'stable') return;
        changeWaiting = false;
        offerInFlight = true;
        try {
            const offer = await connection.createOffer();
            await connection.setLocalDescription(offer);
            onOffer(connection.localDescription?.sdp ?? offer.sdp ?? '');
        } catch (error) {
            offerInFlight = false;
            console.warn('could not offer a change', error);
        }
    };

    connection.onnegotiationneeded = () => {
        changeWaiting = true;
        void offerWaitingChange();
    };

    connection.onconnectionstatechange = () => {
        if (connection.connectionState === 'connected') onConnected();
        else if (connection.connectionState === 'failed') onFailed();
    };

    // After the negotiation's own messages went out: the caller sends the answer once we return.
    const reapplyUplinkSoon = () => {
        uplinkTimer = setTimeout(() => void uplink.reapply(), 0);
    };

    /** A send-only section of our own for one of the screen's tracks. */
    const addScreenSection = (track: MediaStreamTrack, screen: MediaStream): RTCRtpTransceiver => {
        const init: SectionInit = { direction: 'sendonly', streams: [screen] };
        if (track.kind === 'audio') return addAudioSection(connection, track, init);
        return connection.addTransceiver(track, init);
    };

    /** The microphone's section is made directly so it gets its priority and RED; the camera's by `addTrack`. */
    const addInput = (track: MediaStreamTrack, stream: MediaStream): RTCRtpSender => {
        if (track.kind !== 'audio') return connection.addTrack(track, stream);
        return addAudioSection(connection, track, { direction: 'sendrecv', streams: [stream] }).sender;
    };

    // The camera's and the microphone's senders, to switch what they send without a new negotiation.
    const inputSenders = new Map<string, RTCRtpSender>();
    if (localStream) {
        for (const track of localStream.getTracks()) inputSenders.set(track.kind, addInput(track, localStream));
    }

    return {
        statsSource: createSfuStatsSource(connection, () => assignments),
        setUplinkActive: uplink.set,
        createOffer: async () => {
            // Tracks added before the first offer are part of it, not a change to offer later.
            changeWaiting = false;
            const offer = await connection.createOffer();
            await connection.setLocalDescription(offer);
            return connection.localDescription?.sdp ?? offer.sdp ?? '';
        },
        applyAnswer: async (sdp) => {
            await connection.setRemoteDescription({ type: 'answer', sdp });
            firstAnswerApplied = true;
            offerInFlight = false;
            reconcile();
            await candidates.flush();
            reapplyUplinkSoon();
            void offerWaitingChange();
        },
        answerOffer: async (sdp, tracks) => {
            // The server's offer wins over one of ours that is still unanswered: the browser rolls ours
            // back, and the change is offered again after this exchange.
            if (offerInFlight) {
                offerInFlight = false;
                changeWaiting = true;
            }
            await connection.setRemoteDescription({ type: 'offer', sdp });
            // The transceivers and their mids exist now.
            assignments = tracks;
            reconcile();
            await candidates.flush();
            const answer = await connection.createAnswer();
            await connection.setLocalDescription(answer);
            reapplyUplinkSoon();
            const answerSdp = connection.localDescription?.sdp ?? answer.sdp ?? '';
            // A later task, so the caller has sent this answer before any offer of ours goes out.
            setTimeout(() => void offerWaitingChange(), 0);
            return answerSdp;
        },
        addIceCandidate: candidates.add,
        replaceInput: async (track) => {
            await inputSenders.get(track.kind)?.replaceTrack(track);
        },
        shareScreen: (screen) => {
            const [video] = screen.getVideoTracks();
            if (!video || screenSenders.length > 0) return;
            // Never `addTrack`: Chromium would put the screen on any section that has not sent yet, including
            // one the server made to forward someone else's media, and the server never accepts a publisher
            // on its own sending sections. A section of our own is one the server did not create. The picture
            // goes first, so the server has the sharer's camera and microphone before either screen track.
            const tracks = [video, ...screen.getAudioTracks().slice(0, 1)];
            screenSenders = tracks.map((track) => addScreenSection(track, screen).sender);
            // A paused uplink covers the new sender like the others.
            reapplyUplinkSoon();
        },
        stopScreen: () => {
            // The sections stay and go quiet (the server sees them end in the next offer, as before); the
            // tracks themselves are stopped by whoever captured them. A later share gets sections of its own.
            if (connection.signalingState !== 'closed') {
                for (const sender of screenSenders) connection.removeTrack(sender);
            }
            screenSenders = [];
        },
        close: () => {
            connection.onicecandidate = null;
            connection.onnegotiationneeded = null;
            connection.onconnectionstatechange = null;
            if (uplinkTimer) clearTimeout(uplinkTimer);
            uplink.stop();
            connection.close();
        },
    };
};
