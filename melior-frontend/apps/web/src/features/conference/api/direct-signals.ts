import type { ClientMessage, IceCandidate, SessionId } from '@/features/conference/domain';
import { preferVideoCodec } from './codec-preferences';

type DirectSignals = {
    /** Sends the link's offer or answer, then the local candidates that waited for it. */
    sendDescription: (channel: 'peer_offer' | 'peer_answer', sdp: string) => void;
    /** A local candidate: sent at once once the description went out, held until then. */
    localCandidate: (candidate: IceCandidate) => void;
};

/**
 * What a link says to its peer through the server. Local candidates wait until this link's offer or answer went
 * out: the peer has no link yet before it, and would drop a candidate that overtook its description.
 */
export const createDirectSignals = (peerId: SessionId, send: (message: ClientMessage) => void): DirectSignals => {
    let descriptionSent = false;
    const held: IceCandidate[] = [];
    const sendCandidate = (candidate: IceCandidate) =>
        send({ channel: 'peer_ice', payload: { to: peerId, candidate } });

    return {
        sendDescription: (channel, sdp) => {
            send({ channel, payload: { to: peerId, sdp } });
            descriptionSent = true;
            for (const candidate of held.splice(0)) sendCandidate(candidate);
        },
        localCandidate: (candidate) => {
            if (descriptionSent) sendCandidate(candidate);
            else held.push(candidate);
        },
    };
};

/** Creates and sets this side's offer, VP8 only; returns the SDP to send. */
export const makeOffer = async (connection: RTCPeerConnection): Promise<string> => {
    preferVideoCodec(connection);
    const offer = await connection.createOffer();
    await connection.setLocalDescription(offer);
    return connection.localDescription?.sdp ?? offer.sdp ?? '';
};

/** Creates and sets this side's answer, VP8 only; returns the SDP to send. */
export const makeAnswer = async (connection: RTCPeerConnection): Promise<string> => {
    preferVideoCodec(connection);
    const answer = await connection.createAnswer();
    await connection.setLocalDescription(answer);
    return connection.localDescription?.sdp ?? answer.sdp ?? '';
};
