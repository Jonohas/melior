import { type CommunityId, type FeedMessage, parseFeedMessage } from '@/features/communities/domain';
import { sameOriginSocketUrl } from '@/lib/socket-url';

type CommunityFeedOptions = {
    communityId: CommunityId;
    onOpen: () => void;
    onMessage: (message: FeedMessage) => void;
    /** `opened` is false when the socket closed before it ever opened (a refused upgrade shows no status). */
    onClose: (info: { opened: boolean }) => void;
};

export type CommunityFeed = { close: () => void };

/** The community's live feed (`/ws/communities/{id}`): voice roster, text messages and structure changes. */
export const openCommunityFeed = ({ communityId, onOpen, onMessage, onClose }: CommunityFeedOptions): CommunityFeed => {
    const socket = new WebSocket(sameOriginSocketUrl(`/ws/communities/${encodeURIComponent(communityId)}`));
    let opened = false;

    socket.onopen = () => {
        opened = true;
        onOpen();
    };
    socket.onerror = (event) => console.warn('community feed error', event);
    socket.onclose = () => onClose({ opened });
    socket.onmessage = (event) => {
        const message = typeof event.data === 'string' ? parseFeedMessage(event.data) : null;
        if (!message) {
            console.warn('ignoring invalid or unknown community feed message');
            return;
        }
        onMessage(message);
    };

    return {
        close: () => {
            socket.onopen = socket.onclose = socket.onerror = socket.onmessage = null;
            socket.close();
        },
    };
};
