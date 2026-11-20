import {
    type ClientMessage,
    DIRECT_SECTIONS,
    type IceCandidate,
    type IceServer,
    type SessionId,
} from '@/features/conference/domain';
import { addOfferedSections, attachAnsweredSections, type LocalTracks, NO_SENDERS, sectionOf } from './direct-sections';
import { createIceCandidateQueue } from './ice-candidate-queue';
import { createScreenChannel } from './screen-channel';

type DirectLinkOptions = {
    peerId: SessionId;
    /** True when this side makes the offer (the lower session id). */
    offerer: boolean;
    iceServers: IceServer[];
    forceRelay: boolean;
    /** The stream the local microphone and camera belong to. */
    localStream: MediaStream | null;
    /** The tracks this side sends now; read when the link is negotiated, so a link opened later is current. */
    local: () => LocalTracks;
    send: (message: ClientMessage) => void;
    /** The peer's camera and microphone, once the first track arrives. */
    onMedia: (stream: MediaStream) => void;
    /** The peer's screen while it says it shares one (a stream), and null when it stops. */
    onScreen: (stream: MediaStream | null) => void;
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
    /** Sends the screen track (or none) from now on, in place of the previous one; no renegotiation. */
    setScreen: (track: MediaStreamTrack | null) => Promise<void>;
    /** Sends another microphone or camera track from now on; no renegotiation. */
    replaceInput: (track: MediaStreamTrack) => Promise<void>;
    connection: RTCPeerConnection;
    /** The ids of the receiving tracks in section order (audio, camera, screen), for the stats. */
    sectionTrackIds: () => string[];
    isConnected: () => boolean;
    close: () => void;
};

export const createDirectLink = ({
    peerId,
    offerer,
    iceServers,
    forceRelay,
    localStream,
    local,
    send,
    onMedia,
    onScreen,
    onFailed,
}: DirectLinkOptions): DirectLink => {
    const connection = new RTCPeerConnection({ iceServers, iceTransportPolicy: forceRelay ? 'relay' : 'all' });
    const remote = new MediaStream();
    const remoteScreen = new MediaStream();
    let senders = NO_SENDERS;
    let peerShares = false;
    let screenArrived = false;
    const candidates = createIceCandidateQueue(connection, 'direct');
    // The link is negotiated once: a repeated start or offer is ignored.
    let started = false;

    // The peer's screen is shown only while it says it shares one, and its section has delivered a track.
    let published: MediaStream | null = null;
    const publishScreen = () => {
        const next = peerShares && screenArrived ? remoteScreen : null;
        if (next === published) return;
        published = next;
        onScreen(next);
    };

    // Both sides create the data channel before negotiation; it opens with the transport.
    const channel = createScreenChannel(connection, {
        isSharing: () => local().screen !== null,
        onPeerScreen: (sharing) => {
            peerShares = sharing;
            publishScreen();
        },
    });

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
        if (DIRECT_SECTIONS[connection.getTransceivers().indexOf(event.transceiver)] === 'screen') {
            remoteScreen.addTrack(event.track);
            screenArrived = true;
            publishScreen();
            return;
        }
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
            senders = addOfferedSections(connection, local(), localStream);
            const offer = await connection.createOffer();
            await connection.setLocalDescription(offer);
            const sdp = connection.localDescription?.sdp ?? offer.sdp ?? '';
            sendDescription('peer_offer', sdp);
        },
        handleOffer: async (sdp) => {
            if (started) return;
            started = true;
            await connection.setRemoteDescription({ type: 'offer', sdp });
            // After the offer, so the tracks ride the offer's transceivers.
            senders = await attachAnsweredSections(connection, local());
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
        sectionTrackIds: () => connection.getTransceivers().map((t) => t.receiver.track.id),
        setScreen: async (track) => {
            await senders.screen?.replaceTrack(track);
            channel.announce();
        },
        replaceInput: async (track) => {
            await senders[sectionOf(track)]?.replaceTrack(track);
        },
        close: () => {
            connection.onicecandidate = null;
            connection.ontrack = null;
            connection.onconnectionstatechange = null;
            channel.close();
            connection.close();
        },
    };
};
