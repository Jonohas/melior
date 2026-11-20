import { type ClientMessage, type IceServer, isOfferer, type SessionId } from '@/features/conference/domain';
import { createDirectLink, type DirectLink } from './direct-link';
import type { StatsSource } from './stats-sampler';

type LinkRegistryOptions = {
    selfId: SessionId;
    iceServers: IceServer[];
    forceRelay: boolean;
    localStream: MediaStream | null;
    send: (message: ClientMessage) => void;
    /** A peer's direct media arrived (a stream) or went away (null). */
    onMedia: (peer: SessionId, stream: MediaStream | null) => void;
    /** A link reached `failed`. It is already closed and removed, and this is called once per link. */
    onFailed: (peer: SessionId) => void;
};

/** The open direct links, one per peer: opening, closing, and the stats sources of the open ones. */
export type LinkRegistry = {
    /** The link to `peer`, created on first use. */
    open: (peer: SessionId) => DirectLink;
    get: (peer: SessionId) => DirectLink | undefined;
    close: (peer: SessionId) => void;
    closeAll: () => void;
    statsSources: () => StatsSource[];
};

export const createLinkRegistry = ({
    selfId,
    iceServers,
    forceRelay,
    localStream,
    send,
    onMedia,
    onFailed,
}: LinkRegistryOptions): LinkRegistry => {
    const links = new Map<SessionId, DirectLink>();
    const sources = new Map<SessionId, StatsSource>();

    const close = (peer: SessionId) => {
        links.get(peer)?.close();
        links.delete(peer);
        sources.delete(peer);
        onMedia(peer, null);
    };

    const open = (peer: SessionId): DirectLink => {
        const existing = links.get(peer);
        if (existing) return existing;
        const link: DirectLink = createDirectLink({
            peerId: peer,
            offerer: isOfferer(selfId, peer),
            iceServers,
            forceRelay,
            localStream,
            send,
            onMedia: (stream) => onMedia(peer, stream),
            onFailed: () => {
                if (links.get(peer) !== link) return;
                close(peer);
                onFailed(peer);
            },
        });
        links.set(peer, link);
        sources.set(peer, { connection: link.connection, isConnected: link.isConnected, peerOf: () => peer });
        return link;
    };

    return {
        open,
        get: (peer) => links.get(peer),
        close,
        closeAll: () => {
            for (const peer of [...links.keys()]) close(peer);
        },
        statsSources: () => [...sources.values()],
    };
};
