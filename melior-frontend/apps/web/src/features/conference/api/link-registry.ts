import {
    type ClientMessage,
    directKindOf,
    type IceServer,
    isOfferer,
    type SessionId,
} from '@/features/conference/domain';
import { createDirectLink, type DirectLink } from './direct-link';
import type { LocalTracks } from './direct-sections';
import type { StatsSource } from './stats-sampler';

type LinkRegistryOptions = {
    selfId: SessionId;
    iceServers: IceServer[];
    forceRelay: boolean;
    localStream: MediaStream | null;
    /** The screen being shared when the mesh starts (a share survives a reconnect), or none. */
    screenTrack: MediaStreamTrack | null;
    send: (message: ClientMessage) => void;
    /** A peer's direct media arrived (a stream) or went away (null). */
    onMedia: (peer: SessionId, stream: MediaStream | null) => void;
    /** A peer's shared screen arrived over its direct link (a stream) or went away (null). */
    onScreen: (peer: SessionId, stream: MediaStream | null) => void;
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
    /** Sends the screen (or none) on every open link, and on every link opened later. */
    setScreen: (track: MediaStreamTrack | null) => Promise<void>;
    statsSources: () => StatsSource[];
};

export const createLinkRegistry = ({
    selfId,
    iceServers,
    forceRelay,
    localStream,
    screenTrack,
    send,
    onMedia,
    onScreen,
    onFailed,
}: LinkRegistryOptions): LinkRegistry => {
    // What a link opened now sends; kept current by `setScreen` and `replaceInput`.
    const local: LocalTracks = {
        audio: localStream?.getAudioTracks()[0] ?? null,
        video: localStream?.getVideoTracks()[0] ?? null,
        screen: screenTrack,
    };
    const links = new Map<SessionId, DirectLink>();
    const sources = new Map<SessionId, StatsSource>();

    const close = (peer: SessionId) => {
        links.get(peer)?.close();
        links.delete(peer);
        sources.delete(peer);
        onMedia(peer, null);
        onScreen(peer, null);
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
            local: () => local,
            send,
            onMedia: (stream) => onMedia(peer, stream),
            onScreen: (stream) => onScreen(peer, stream),
            onFailed: () => {
                if (links.get(peer) !== link) return;
                close(peer);
                onFailed(peer);
            },
        });
        links.set(peer, link);
        sources.set(peer, {
            connection: link.connection,
            isConnected: link.isConnected,
            sourceOf: (entry) => {
                const kind = directKindOf(entry, link.sectionTrackIds());
                return kind === undefined ? undefined : { peer, kind };
            },
        });
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
        setScreen: async (track) => {
            local.screen = track;
            await Promise.all([...links.values()].map((link) => link.setScreen(track)));
        },
    };
};
