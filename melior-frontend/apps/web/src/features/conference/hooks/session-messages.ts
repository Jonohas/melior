import type { Dispatch } from 'react';
import {
    createFrameTransforms,
    createPeerSession,
    type E2eeSession,
    type FrameTransforms,
    type Mesh,
    type PeerSession,
    type SignallingClient,
    sharedDeviceStore,
    sharedVoucher,
    startE2eeSession,
} from '@/features/conference/api';
import type { CloseReason, ConferenceAction, ServerMessage, TrackAssignment } from '@/features/conference/domain';
import { startMesh } from './start-mesh';

/** What the room says in chat: a message for everyone, or a refusal of the one this client sent. */
export type ChatEvent = Extract<ServerMessage, { channel: 'chat' | 'chat_error' }>;

/** What one signalling connection owns; every field is replaced on reconnect. */
export type ConferenceConnection = {
    signalling: SignallingClient | null;
    peer: PeerSession | null;
    mesh: Mesh | null;
    /** The frame worker and its transforms; every connection's media goes through them. */
    transforms: FrameTransforms | null;
    /** The key exchange with the other participants. */
    e2ee: E2eeSession | null;
    /**
     * Key work that arrived while this call's keys were still being created: a peer's hello, a join, a leave.
     * Run in order once the exchange exists; null when nothing is starting.
     */
    e2eeWaiting: Array<(e2ee: E2eeSession) => void> | null;
};

/** Runs `action` on the key exchange now, or as soon as it exists. Dropped when no call is starting. */
export const withE2ee = (connection: ConferenceConnection, action: (e2ee: E2eeSession) => void): void => {
    if (connection.e2ee) action(connection.e2ee);
    else connection.e2eeWaiting?.push(action);
};

export const releaseConnection = (connection: ConferenceConnection): void => {
    connection.signalling?.close();
    connection.mesh?.close();
    connection.peer?.close();
    connection.e2ee?.close();
    connection.transforms?.close();
    connection.signalling = null;
    connection.peer = null;
    connection.mesh = null;
    connection.e2ee = null;
    connection.e2eeWaiting = null;
    connection.transforms = null;
};

/** What the handlers need from the hook; created per message, so `isCurrent` pins the connection's generation. */
export type MessageContext = {
    connection: ConferenceConnection;
    dispatch: Dispatch<ConferenceAction<MediaStream>>;
    forceRelay: boolean;
    localStream: MediaStream | null;
    /** The screen shared now, or none; a mesh started on a new connection sends it too. */
    screenStream: () => MediaStream | null;
    /** False once the connection this message belongs to was torn down; late callbacks then do nothing. */
    isCurrent: () => boolean;
    onConnected: () => void;
    onChat: (event: ChatEvent) => void;
    onPeerFailed: () => void;
    /** The server announced it is closing the socket. */
    onServerClosed: (reason: CloseReason) => void;
    /** The server sent an `error`: the connection is useless. */
    onServerError: (message: string) => void;
    /** Something this client needs failed for good, such as the encryption keys; the call stops. */
    onFatal: (message: string) => void;
};

type SessionMessage = Extract<ServerMessage, { channel: 'session' }>['payload'];

/**
 * Creates this call's keys and starts the key exchange with everyone already in the room. Media flows only after
 * this: every sender and receiver is created with its transform.
 */
const startEncryption = async (
    context: MessageContext,
    payload: SessionMessage,
    transforms: FrameTransforms,
): Promise<E2eeSession> => {
    const { connection, dispatch, isCurrent } = context;
    const device = await (await sharedDeviceStore()).deviceKey();
    const e2ee = await startE2eeSession({
        selfId: payload.session_id,
        device,
        voucher: await sharedVoucher(device),
        post: transforms.post,
        send: (to, message) => {
            if (isCurrent()) connection.signalling?.send({ channel: 'e2ee', payload: { to, payload: message } });
        },
        onTrust: (sessionId, trust) => {
            if (isCurrent()) dispatch({ type: 'trust_changed', sessionId, trust });
        },
    });
    for (const participant of payload.participants) e2ee.addPeer(participant.session_id, participant.username, false);
    return e2ee;
};

/** The `session` message: starts encryption, creates the SFU connection and the mesh, and sends the first offer. */
export const startSession = async (context: MessageContext, payload: SessionMessage): Promise<void> => {
    const { connection, dispatch, isCurrent } = context;
    if (connection.transforms) {
        console.warn('duplicate session message ignored');
        return;
    }
    const transforms = createFrameTransforms();
    connection.transforms = transforms;
    connection.e2eeWaiting = [];
    dispatch({
        type: 'session_started',
        sessionId: payload.session_id,
        username: payload.username,
        participants: payload.participants,
    });
    try {
        const e2ee = await startEncryption(context, payload, transforms);
        if (!isCurrent()) return e2ee.close();
        connection.e2ee = e2ee;
        for (const action of connection.e2eeWaiting?.splice(0) ?? []) action(e2ee);
        connection.e2eeWaiting = null;
    } catch (error) {
        if (isCurrent()) context.onFatal(`End-to-end encryption could not start: ${String(error)}`);
        return;
    }
    const session = createPeerSession({
        iceServers: payload.ice_servers,
        forceRelay: context.forceRelay,
        localStream: context.localStream,
        protect: transforms.protect,
        onIceCandidate: (candidate) => {
            if (isCurrent()) connection.signalling?.send({ channel: 'ice', payload: candidate });
        },
        onOffer: (sdp) => {
            if (isCurrent()) connection.signalling?.send({ channel: 'sdp_offer', payload: { type: 'offer', sdp } });
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
        protect: transforms.protect,
        screen: context.screenStream(),
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
        case 'e2ee':
            return withE2ee(connection, (e2ee) => void e2ee.handle(message.payload.from, message.payload.payload));
        case 'peer_joined':
            withE2ee(connection, (e2ee) => e2ee.addPeer(message.payload.session_id, message.payload.username, true));
            return dispatch({
                type: 'peer_joined',
                sessionId: message.payload.session_id,
                username: message.payload.username,
                voice: message.payload.voice,
            });
        case 'voice_state': {
            const { session_id, ...voice } = message.payload;
            return dispatch({ type: 'voice_changed', sessionId: session_id, voice });
        }
        case 'peer_left':
            connection.mesh?.peerLeft(message.payload.session_id);
            withE2ee(connection, (e2ee) => e2ee.removePeer(message.payload.session_id));
            return dispatch({ type: 'peer_left', sessionId: message.payload.session_id });
        case 'chat':
        case 'chat_error':
            return context.onChat(message);
        case 'closed':
            return context.onServerClosed(message.payload.reason);
        case 'error':
            return context.onServerError(message.payload.message);
    }
};
