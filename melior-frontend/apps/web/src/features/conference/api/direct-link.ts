import {
    type ClientMessage,
    DIRECT_SECTIONS,
    type DirectSection,
    type IceCandidate,
    type IceServer,
    type SessionId,
} from '@/features/conference/domain';
import {
    addOfferedSections,
    attachAnsweredSections,
    type LocalTracks,
    NO_SENDERS,
    type ScreenTracks,
    type SectionSenders,
    sectionOf,
} from './direct-sections';
import { createDirectSignals, makeAnswer, makeOffer } from './direct-signals';
import { createIceCandidateQueue } from './ice-candidate-queue';
import { createPeerScreen } from './peer-screen';
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
    /** Sends the screen's tracks (or none) from now on, in place of the previous ones; no renegotiation. */
    setScreen: (tracks: ScreenTracks) => Promise<void>;
    /** Sends another microphone or camera track from now on; no renegotiation. */
    replaceInput: (track: MediaStreamTrack) => Promise<void>;
    connection: RTCPeerConnection;
    /** The ids of the receiving tracks in section order (audio, camera, screen, screen audio), for the stats. */
    sectionTrackIds: () => string[];
    isConnected: () => boolean;
    close: () => void;
};

/** The sections that carry the peer's screen: its picture and its sound. */
const isScreenSection = (section: DirectSection | undefined): boolean =>
    section === 'screen' || section === 'screenAudio';

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
    const peerScreen = createPeerScreen(onScreen);
    const signals = createDirectSignals(peerId, send);
    const candidates = createIceCandidateQueue(connection, 'direct');
    // Filled in as the sections are attached, so a change during the attach finds the senders already there.
    const senders: SectionSenders = { ...NO_SENDERS };
    // The link is negotiated once: a repeated start or offer is ignored.
    let started = false;

    // Both sides create the data channel before negotiation; it opens with the transport.
    const channel = createScreenChannel(connection, {
        isSharing: () => local().screen !== null,
        onPeerScreen: peerScreen.setSharing,
    });

    connection.onicecandidate = (event) => {
        if (event.candidate) signals.localCandidate(event.candidate.toJSON());
    };
    connection.ontrack = (event) => {
        if (isScreenSection(DIRECT_SECTIONS[connection.getTransceivers().indexOf(event.transceiver)])) {
            peerScreen.trackArrived(event.track);
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
            Object.assign(senders, addOfferedSections(connection, local(), localStream));
            signals.sendDescription('peer_offer', await makeOffer(connection));
        },
        handleOffer: async (sdp) => {
            if (started) return;
            started = true;
            await connection.setRemoteDescription({ type: 'offer', sdp });
            // After the offer, so the tracks ride the offer's transceivers.
            await attachAnsweredSections(connection, local, senders);
            await candidates.flush();
            signals.sendDescription('peer_answer', await makeAnswer(connection));
        },
        handleAnswer: async (sdp) => {
            await connection.setRemoteDescription({ type: 'answer', sdp });
            await candidates.flush();
        },
        addIceCandidate: (candidate: IceCandidate) => candidates.add(candidate),
        sectionTrackIds: () => connection.getTransceivers().map((t) => t.receiver.track.id),
        setScreen: async (tracks) => {
            try {
                await Promise.all([
                    senders.screen?.replaceTrack(tracks.screen),
                    senders.screenAudio?.replaceTrack(tracks.screenAudio),
                ]);
            } finally {
                // The peer learns the new state even when this link's sender could not take the track.
                channel.announce();
            }
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
