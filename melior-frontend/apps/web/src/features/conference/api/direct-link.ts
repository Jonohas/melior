import type { ClientMessage, IceCandidate, IceServer, SessionId } from '@/features/conference/domain';

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
    let candidateQueue: IceCandidate[] = [];

    const addLocalTracks = () => {
        if (!localStream) return;
        for (const track of localStream.getTracks()) connection.addTrack(track, localStream);
    };

    connection.onicecandidate = (event) => {
        if (event.candidate)
            send({ channel: 'peer_ice', payload: { to: peerId, candidate: event.candidate.toJSON() } });
    };
    connection.ontrack = (event) => {
        remote.addTrack(event.track);
        onMedia(remote);
    };
    connection.onconnectionstatechange = () => {
        if (connection.connectionState === 'failed') onFailed();
    };

    const addCandidate = async (candidate: IceCandidate) => {
        try {
            await connection.addIceCandidate(candidate);
        } catch (error) {
            console.warn('direct addIceCandidate failed', error);
        }
    };

    const flushCandidates = async () => {
        const queued = candidateQueue;
        candidateQueue = [];
        for (const candidate of queued) await addCandidate(candidate);
    };

    return {
        connection,
        isConnected: () => connection.connectionState === 'connected',
        start: async () => {
            if (!offerer) return;
            addLocalTracks();
            const offer = await connection.createOffer();
            await connection.setLocalDescription(offer);
            const sdp = connection.localDescription?.sdp ?? offer.sdp ?? '';
            send({ channel: 'peer_offer', payload: { to: peerId, sdp } });
        },
        handleOffer: async (sdp) => {
            await connection.setRemoteDescription({ type: 'offer', sdp });
            // After the offer, so the tracks reuse the offer's transceivers.
            addLocalTracks();
            await flushCandidates();
            const answer = await connection.createAnswer();
            await connection.setLocalDescription(answer);
            const answerSdp = connection.localDescription?.sdp ?? answer.sdp ?? '';
            send({ channel: 'peer_answer', payload: { to: peerId, sdp: answerSdp } });
        },
        handleAnswer: async (sdp) => {
            await connection.setRemoteDescription({ type: 'answer', sdp });
            await flushCandidates();
        },
        addIceCandidate: async (candidate) => {
            if (!connection.remoteDescription) {
                candidateQueue.push(candidate);
                return;
            }
            await addCandidate(candidate);
        },
        close: () => {
            connection.onicecandidate = null;
            connection.ontrack = null;
            connection.onconnectionstatechange = null;
            connection.close();
        },
    };
};
