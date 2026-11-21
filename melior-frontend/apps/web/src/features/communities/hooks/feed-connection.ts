import { openCommunityFeed } from '@/features/communities/api';
import {
    type CommunityId,
    decideFeedClose,
    type FeedMessage,
    type RemovedReason,
    type RequestFailure,
    removalAfterProbe,
} from '@/features/communities/domain';
import { nextAttempt, reconnectDelayMs } from '@/features/conference/domain';

/** Where the community feed stands. `stopped` follows a removal: there is nothing to reconnect to. */
export type FeedStatus = 'connecting' | 'open' | 'reconnecting' | 'stopped';

type FeedConnectionDeps = {
    communityId: CommunityId;
    onStatus: (status: FeedStatus) => void;
    onMessage: (message: FeedMessage) => void;
    /** Fetches the community again; resolves to how that failed, or null when it worked. */
    checkMembership: () => Promise<RequestFailure | null>;
    onRemoved: (reason: RemovedReason) => void;
};

/**
 * Keeps the community feed open: after an unexpected loss it reconnects with the conference's backoff (1 s doubling
 * to 15 s, with jitter, restarted by a connection that opened). A socket that never opened may mean the user is
 * no longer a member, so the community is fetched first; a 403 or 404 there ends it.
 */
export const createFeedConnection = (deps: FeedConnectionDeps): { dispose: () => void } => {
    let disposed = false;
    let removed = false;
    let sessionEnded = false;
    let attempt = 0;
    let reachedOpen = false;
    let timer: ReturnType<typeof setTimeout> | null = null;
    let feed: { close: () => void } | null = null;

    const stop = (reason: RemovedReason) => {
        removed = true;
        feed?.close();
        deps.onStatus('stopped');
        deps.onRemoved(reason);
    };

    const scheduleReconnect = () => {
        if (disposed || removed) return;
        attempt = nextAttempt(attempt, reachedOpen);
        reachedOpen = false;
        deps.onStatus('reconnecting');
        timer = setTimeout(connect, reconnectDelayMs(attempt, Math.random()));
    };

    const checkMembership = async () => {
        const removal = removalAfterProbe(await deps.checkMembership());
        if (disposed || removed) return;
        if (removal) stop(removal);
        else scheduleReconnect();
    };

    const handle = (message: FeedMessage) => {
        if (message.channel === 'removed') removed = true;
        if (message.channel === 'closed') sessionEnded = true;
        deps.onMessage(message);
    };

    function connect() {
        if (disposed) return;
        feed = openCommunityFeed({
            communityId: deps.communityId,
            onOpen: () => {
                reachedOpen = true;
                deps.onStatus('open');
            },
            onMessage: handle,
            onClose: ({ opened }) => {
                if (disposed) return;
                const decision = decideFeedClose({ removed, sessionEnded, opened });
                sessionEnded = false;
                if (decision === 'check_membership') void checkMembership();
                if (decision === 'reconnect') scheduleReconnect();
            },
        });
    }

    deps.onStatus('connecting');
    connect();

    return {
        dispose: () => {
            disposed = true;
            if (timer) clearTimeout(timer);
            feed?.close();
        },
    };
};
