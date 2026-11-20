import type { ClientMessage, IceCandidate, IceServer, SessionId } from '@/features/conference/domain';
import { createIceCandidateQueue } from './ice-candidate-queue';

type DirectLinkOptions = {
    peerId: SessionId;
    /** True when this side makes the offer (the lower session id). */
    offerer: boolean;
    iceServers: IceServer[];
    forceRelay: boolean;
    localStream: MediaStream | null;
    send: (message: ClientMessage) => void;
    /** The peer's media, once the first track arrives. */
    onMedia: (stream: MediaStream) => void;
    /** The link reached `failed`; the owner closes it and does nothing else. */
    onFailed: () => void;
};

/** One browser-to-browser connection with one peer, negotiated once. */
export type DirectLink = {
    /** The offerer starts the negotiation; the answerer does nothing until the offer arrives. */
    start: () => Promise<void>;
    handleOffer: (sdp: string) => Promise<void>;
    handleAnswer: (sdp: string) => Promise<void>;
    addIceCandidate: (candidate: IceCandidate) => Promise<void>;
    connection: RTCPeerConnection;
    isConnected: () => boolean;
    close: () => void;
};

export const createDirectLink = ({
    peerId,
    offerer,
    iceServers,
    forceRelay,
    localStream,
    send,
    onMedia,
    onFailed,
}: DirectLinkOptions): DirectLink => {
    const connection = new RTCPeerConnection({ iceServers, iceTransportPolicy: forceRelay ? 'relay' : 'all' });
    const remote = new MediaStream();
    const candidates = createIceCandidateQueue(connection, 'direct');
    // The link is negotiated once: a repeated start or offer is ignored.
    let started = false;

    const addLocalTracks = () => {
        if (!localStream) return;
        for (const track of localStream.getTracks()) connection.addTrack(track, localStream);
    };

    // Local candidates wait until this link's offer or answer went out: the peer has no link yet before it,
    // and would drop a candidate that overtook its description.
    let descriptionSent = false;
    const heldCandidates: IceCandidate[] = [];
    const sendCandidate = (candidate: IceCandidate) =>
        send({ channel: 'peer_ice', payload: { to: peerId, candidate } });

    const sendDescription = (channel: 'peer_offer' | 'peer_answer', sdp: string) => {
        send({ channel, payload: { to: peerId, sdp } });
        descriptionSent = true;
        for (const candidate of heldCandidates.splice(0)) sendCandidate(candidate);
    };

    connection.onicecandidate = (event) => {
        if (!event.candidate) return;
        const candidate = event.candidate.toJSON();
        if (descriptionSent) sendCandidate(candidate);
        else heldCandidates.push(candidate);
    };
    connection.ontrack = (event) => {
        remote.addTrack(event.track);
        onMedia(remote);
    };
    connection.onconnectionstatechange = () => {
        if (connection.connectionState === 'failed') onFailed();
    };

    return {
        connection,
        isConnected: () => connection.connectionState === 'connected',
        start: async () => {
            if (!offerer || started) return;
            started = true;
            addLocalTracks();
            const offer = await connection.createOffer();
            await connection.setLocalDescription(offer);
            const sdp = connection.localDescription?.sdp ?? offer.sdp ?? '';
            sendDescription('peer_offer', sdp);
        },
        handleOffer: async (sdp) => {
            if (started) return;
            started = true;
            await connection.setRemoteDescription({ type: 'offer', sdp });
            // After the offer, so the tracks reuse the offer's transceivers.
            addLocalTracks();
            await candidates.flush();
            const answer = await connection.createAnswer();
            await connection.setLocalDescription(answer);
            const answerSdp = connection.localDescription?.sdp ?? answer.sdp ?? '';
            sendDescription('peer_answer', answerSdp);
        },
        handleAnswer: async (sdp) => {
            await connection.setRemoteDescription({ type: 'answer', sdp });
            await candidates.flush();
        },
        addIceCandidate: (candidate: IceCandidate) => candidates.add(candidate),
        close: () => {
            connection.onicecandidate = null;
            connection.ontrack = null;
            connection.onconnectionstatechange = null;
            connection.close();
        },
    };
};
