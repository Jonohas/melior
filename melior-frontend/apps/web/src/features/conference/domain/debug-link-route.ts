import type { PeerRouteView } from './source-selection';

/** A link's route as the server names it (README, "Mesh"). */
export type LinkRoute = 'Forwarded' | 'ToDirect' | 'Direct' | 'ToForwarded';

type RouteFacts = {
    view: PeerRouteView;
    /** Whether this side has a direct link to the peer open. */
    linkOpen: boolean;
    /** Whether the SFU still sends this side the peer's tracks (the server's last `tracks` list names the peer). */
    sfuCarriesPeer: boolean;
};

/**
 * The server does not send a link's route, only the `route` messages that change it; this is what the client can
 * tell from them. `Direct` once this side reported the direct route live and the SFU stopped sending the peer's
 * tracks (the server unsubscribes both sides only after both reported). `ToForwarded` while a forwarded route
 * still has its direct link open (until `link_close`).
 */
export const inferLinkRoute = ({ view, linkOpen, sfuCarriesPeer }: RouteFacts): LinkRoute => {
    if (view.preferred === 'direct') {
        const reported = view.epoch !== null && view.reportedEpoch === view.epoch;
        return reported && !sfuCarriesPeer ? 'Direct' : 'ToDirect';
    }
    return view.epoch !== null && linkOpen ? 'ToForwarded' : 'Forwarded';
};
