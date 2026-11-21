import {
    applyRoute,
    type ClientMessage,
    type IceServer,
    initialRouteView,
    isOfferer,
    NO_OBSERVATION,
    type Path,
    type PeerRouteView,
    type ServerMessage,
    type SessionId,
    selectSource,
} from '@/features/conference/domain';
import { createLinkRegistry } from './link-registry';
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
    /** The screen being shared when the mesh starts, or none. */
    screen: MediaStream | null;
    /** The SFU connection, for sampling the forwarded path. */
    sfu: () => StatsSource | null;
    send: (message: ClientMessage) => void;
    onDirectMedia: (peer: SessionId, stream: MediaStream | null) => void;
    onDirectScreen: (peer: SessionId, stream: MediaStream | null) => void;
    onShownPath: (peer: SessionId, path: Path) => void;
};

export type Mesh = {
    handle: (message: MeshMessage) => Promise<void>;
    /** Closes the direct link to a peer and forgets its route. */
    peerLeft: (peer: SessionId) => void;
    /** Starts (a stream) or stops (null) sending the screen and its sound on every direct link, without renegotiation. */
    setScreen: (screen: MediaStream | null) => Promise<void>;
    /** Sends another microphone or camera track on every direct link, without renegotiation. */
    replaceInput: (track: MediaStreamTrack) => Promise<void>;
    close: () => void;
};

/**
 * The client half of the mesh: the route each peer was last told and the path each peer is shown over. The
 * direct links live in the registry. Every stats sample runs the pure source selection rule and reports
 * `route_live` when it says so.
 */
export const createMesh = (options: MeshOptions): Mesh => {
    const { selfId, sfu, send, onShownPath } = options;
    const views = new Map<SessionId, PeerRouteView>();

    const storeView = (peer: SessionId, view: PeerRouteView) => {
        if (views.get(peer)?.shown !== view.shown) onShownPath(peer, view.shown);
        views.set(peer, view);
    };

    const showForwarded = (peer: SessionId) => {
        const view = views.get(peer);
        if (view) storeView(peer, { ...view, shown: 'forwarded' });
    };

    const registry = createLinkRegistry({
        ...options,
        onMedia: options.onDirectMedia,
        onScreen: options.onDirectScreen,
        onFailed: (peer) => {
            showForwarded(peer);
            send({ channel: 'link_failed', payload: { peer } });
        },
    });

    const onRoute = async (peer: SessionId, path: Path, epoch: number) => {
        storeView(peer, applyRoute(views.get(peer) ?? initialRouteView(), { path, epoch }));
        if (path === 'direct') await registry.open(peer).start();
    };

    const onLinkClose = (peer: SessionId) => {
        registry.close(peer);
        showForwarded(peer);
    };

    // An offer may beat the route; the offerer is the other side, so only that side's offer opens a link.
    const onOffer = async (from: SessionId, sdp: string) => {
        if (isOfferer(selfId, from)) return;
        await registry.open(from).handleOffer(sdp);
    };

    const onSample = (observations: Observations) => {
        for (const [peer, view] of views) {
            const paths = observations.get(peer) ?? { forwarded: NO_OBSERVATION, direct: NO_OBSERVATION };
            const result = selectSource(view, paths);
            storeView(peer, result.view);
            if (result.report !== null) send({ channel: 'route_live', payload: { peer, epoch: result.report } });
        }
    };

    const sampler = createStatsSampler({ sfu, direct: registry.statsSources, onSample });

    const dispatch = async (message: MeshMessage) => {
        switch (message.channel) {
            case 'route':
                return onRoute(message.payload.peer, message.payload.path, message.payload.epoch);
            case 'link_close':
                return onLinkClose(message.payload.peer);
            case 'peer_offer':
                return onOffer(message.payload.from, message.payload.sdp);
            case 'peer_answer':
                return registry.get(message.payload.from)?.handleAnswer(message.payload.sdp);
            case 'peer_ice':
                return registry.get(message.payload.from)?.addIceCandidate(message.payload.candidate);
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
            registry.close(peer);
            views.delete(peer);
        },
        setScreen: registry.setScreen,
        replaceInput: registry.replaceInput,
        close: () => {
            sampler.stop();
            registry.closeAll();
            views.clear();
        },
    };
};
