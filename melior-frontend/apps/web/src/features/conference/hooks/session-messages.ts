import type { Dispatch } from 'react';
import { createPeerSession, type Mesh, type PeerSession, type SignallingClient } from '@/features/conference/api';
import type { CloseReason, ConferenceAction, ServerMessage, TrackAssignment } from '@/features/conference/domain';
import { startMesh } from './start-mesh';

/** What one signalling connection owns; every field is replaced on reconnect. */
export type ConferenceConnection = {
    signalling: SignallingClient | null;
    peer: PeerSession | null;
    mesh: Mesh | null;
};

export const releaseConnection = (connection: ConferenceConnection): void => {
    connection.signalling?.close();
    connection.mesh?.close();
    connection.peer?.close();
    connection.signalling = null;
    connection.peer = null;
    connection.mesh = null;
};

/** What the handlers need from the hook; created per message, so `isCurrent` pins the connection's generation. */
export type MessageContext = {
    connection: ConferenceConnection;
    dispatch: Dispatch<ConferenceAction<MediaStream>>;
    forceRelay: boolean;
    localStream: MediaStream | null;
    /** False once the connection this message belongs to was torn down; late callbacks then do nothing. */
    isCurrent: () => boolean;
    onConnected: () => void;
    onPeerFailed: () => void;
    /** The server announced it is closing the socket. */
    onServerClosed: (reason: CloseReason) => void;
    /** The server sent an `error`: the connection is useless. */
    onServerError: (message: string) => void;
};

type SessionMessage = Extract<ServerMessage, { channel: 'session' }>['payload'];

/** The `session` message: creates the SFU connection and the mesh, and sends the first offer. */
export const startSession = async (context: MessageContext, payload: SessionMessage): Promise<void> => {
    const { connection, dispatch, isCurrent } = context;
    if (connection.peer) {
        console.warn('duplicate session message ignored');
        return;
    }
    dispatch({
        type: 'session_started',
        sessionId: payload.session_id,
        username: payload.username,
        participants: payload.participants,
    });
    const session = createPeerSession({
        iceServers: payload.ice_servers,
        forceRelay: context.forceRelay,
        localStream: context.localStream,
        onIceCandidate: (candidate) => {
            if (isCurrent()) connection.signalling?.send({ channel: 'ice', payload: candidate });
        },
        onRemoteMedia: (streams) => {
            if (isCurrent()) dispatch({ type: 'remote_media_updated', streams });
        },
        onConnected: () => {
            if (!isCurrent()) return;
            context.onConnected();
            dispatch({ type: 'connected' });
        },
        onFailed: () => {
            if (!isCurrent()) return;
            console.warn('peer connection failed, reconnecting');
            context.onPeerFailed();
        },
    });
    connection.peer = session;
    connection.mesh = startMesh({
        selfId: payload.session_id,
        iceServers: payload.ice_servers,
        forceRelay: context.forceRelay,
        localStream: context.localStream,
        peer: () => connection.peer,
        signalling: () => connection.signalling,
        dispatch,
        isCurrent,
    });
    const sdp = await session.createOffer();
    if (!isCurrent()) return;
    connection.signalling?.send({ channel: 'sdp_offer', payload: { type: 'offer', sdp } });
};

/** The server's offer after a renegotiation: answer it, with the list of who publishes which section. */
export const answerServerOffer = async (
    context: MessageContext,
    offer: { sdp: string; tracks: TrackAssignment[] },
): Promise<void> => {
    const { peer } = context.connection;
    if (!peer) return;
    const sdp = await peer.answerOffer(offer.sdp, offer.tracks);
    if (!context.isCurrent()) return;
    context.connection.signalling?.send({ channel: 'sdp_answer', payload: { type: 'answer', sdp } });
};

/** One server message, routed to the part that owns it. */
export const handleServerMessage = async (context: MessageContext, message: ServerMessage): Promise<void> => {
    const { connection, dispatch } = context;
    switch (message.channel) {
        case 'session':
            return startSession(context, message.payload);
        case 'sdp_answer':
            return connection.peer?.applyAnswer(message.payload.sdp);
        case 'sdp_offer':
            return answerServerOffer(context, message.payload);
        case 'ice':
            return connection.peer?.addIceCandidate(message.payload);
        case 'uplink':
            return connection.peer?.setUplinkActive(message.payload.active);
        case 'route':
        case 'link_close':
        case 'peer_offer':
        case 'peer_answer':
        case 'peer_ice':
            return connection.mesh?.handle(message);
        case 'peer_joined':
            return dispatch({
                type: 'peer_joined',
                sessionId: message.payload.session_id,
                username: message.payload.username,
            });
        case 'peer_left':
            connection.mesh?.peerLeft(message.payload.session_id);
            return dispatch({ type: 'peer_left', sessionId: message.payload.session_id });
        case 'closed':
            return context.onServerClosed(message.payload.reason);
        case 'error':
            return context.onServerError(message.payload.message);
    }
};
