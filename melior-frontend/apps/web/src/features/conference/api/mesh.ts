import {
    applyRoute,
    type ClientMessage,
    type IceServer,
    initialRouteView,
    isOfferer,
    type Path,
    type PathObservation,
    type PeerRouteView,
    type ServerMessage,
    type SessionId,
    selectSource,
} from '@/features/conference/domain';
import { createDirectLink, type DirectLink } from './direct-link';
import { createStatsSampler, type Observations, type StatsSource } from './stats-sampler';

/** The server messages the mesh handles; the hook passes exactly these on. */
export type MeshMessage = Extract<
    ServerMessage,
    { channel: 'route' | 'link_close' | 'peer_offer' | 'peer_answer' | 'peer_ice' }
>;

type MeshOptions = {
    selfId: SessionId;
    iceServers: IceServer[];
    forceRelay: boolean;
    localStream: MediaStream | null;
    /** The SFU connection, for sampling the forwarded path. */
    sfu: () => StatsSource | null;
    send: (message: ClientMessage) => void;
    onDirectMedia: (peer: SessionId, stream: MediaStream | null) => void;
    onShownPath: (peer: SessionId, path: Path) => void;
};

export type Mesh = {
    handle: (message: MeshMessage) => Promise<void>;
    /** Closes the direct link to a peer and forgets its route. */
    peerLeft: (peer: SessionId) => void;
    close: () => void;
};

const NOTHING: PathObservation = { connected: false, flowing: [] };

/**
 * The client half of the mesh: one direct link per peer with an open link, the route each peer was last
 * told, and the path each peer is shown over. Every stats sample runs the pure source selection rule and
 * reports `route_live` when it says so.
 */
export const createMesh = ({
    selfId,
    iceServers,
    forceRelay,
    localStream,
    sfu,
    send,
    onDirectMedia,
    onShownPath,
}: MeshOptions): Mesh => {
    const views = new Map<SessionId, PeerRouteView>();
    const links = new Map<SessionId, DirectLink>();
    const sources = new Map<SessionId, StatsSource>();

    const storeView = (peer: SessionId, view: PeerRouteView) => {
        if (views.get(peer)?.shown !== view.shown) onShownPath(peer, view.shown);
        views.set(peer, view);
    };

    const closeLink = (peer: SessionId) => {
        links.get(peer)?.close();
        links.delete(peer);
        sources.delete(peer);
        onDirectMedia(peer, null);
    };

    const openLink = (peer: SessionId): DirectLink => {
        const existing = links.get(peer);
        if (existing) return existing;
        const link: DirectLink = createDirectLink({
            peerId: peer,
            offerer: isOfferer(selfId, peer),
            iceServers,
            forceRelay,
            localStream,
            send,
            onMedia: (stream) => onDirectMedia(peer, stream),
            // Close it and tell the server, which moves the room to forwarding. The guard makes it once per link.
            onFailed: () => {
                if (links.get(peer) !== link) return;
                closeLink(peer);
                send({ channel: 'link_failed', payload: { peer } });
            },
        });
        links.set(peer, link);
        sources.set(peer, { connection: link.connection, isConnected: link.isConnected, peerOfMid: () => peer });
        return link;
    };

    const onRoute = async (peer: SessionId, path: Path, epoch: number) => {
        storeView(peer, applyRoute(views.get(peer) ?? initialRouteView(), { path, epoch }));
        if (path === 'direct') await openLink(peer).start();
    };

    const onLinkClose = (peer: SessionId) => {
        closeLink(peer);
        const view = views.get(peer);
        if (view) storeView(peer, { ...view, shown: 'forwarded' });
    };

    // An offer may beat the route; the offerer is the other side, so only that side's offer opens a link.
    const onOffer = async (from: SessionId, sdp: string) => {
        if (isOfferer(selfId, from)) return;
        await openLink(from).handleOffer(sdp);
    };

    const onSample = (observations: Observations) => {
        for (const [peer, view] of views) {
            const paths = observations.get(peer) ?? { forwarded: NOTHING, direct: NOTHING };
            const result = selectSource(view, paths);
            storeView(peer, result.view);
            if (result.report !== null) send({ channel: 'route_live', payload: { peer, epoch: result.report } });
        }
    };

    const sampler = createStatsSampler({ sfu, direct: () => [...sources.values()], onSample });

    const dispatch = async (message: MeshMessage) => {
        switch (message.channel) {
            case 'route':
                return onRoute(message.payload.peer, message.payload.path, message.payload.epoch);
            case 'link_close':
                return onLinkClose(message.payload.peer);
            case 'peer_offer':
                return onOffer(message.payload.from, message.payload.sdp);
            case 'peer_answer':
                return links.get(message.payload.from)?.handleAnswer(message.payload.sdp);
            case 'peer_ice':
                return links.get(message.payload.from)?.addIceCandidate(message.payload.candidate);
        }
    };

    return {
        handle: async (message) => {
            try {
                await dispatch(message);
            } catch (error) {
                console.warn(`direct link: error handling "${message.channel}"`, error);
            }
        },
        peerLeft: (peer) => {
            closeLink(peer);
            views.delete(peer);
        },
        close: () => {
            sampler.stop();
            for (const peer of [...links.keys()]) closeLink(peer);
            views.clear();
        },
    };
};
