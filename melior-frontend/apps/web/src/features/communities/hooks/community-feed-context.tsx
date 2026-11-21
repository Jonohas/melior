import { useQueryClient } from '@tanstack/react-query';
import { createContext, type ReactNode, useCallback, useContext, useEffect, useRef, useState } from 'react';
import type { ChatMessage } from '@/features/chat/domain';
import {
    applyToRoster,
    type CommunityId,
    CommunityRequestError,
    EMPTY_ROSTER,
    type FeedMessage,
    type RemovedReason,
    type VoiceRoster,
} from '@/features/communities/domain';
import { queryKeys } from '@/lib/query-keys';
import { createFeedConnection, type FeedStatus } from './feed-connection';
import { communityQueryOptions } from './queries';

type MessageListener = (channelId: string, message: ChatMessage) => void;

export type CommunityFeedValue = {
    status: FeedStatus;
    /** Who is in which voice channel. */
    roster: VoiceRoster;
    /** Listens for text channel messages; returns the function that stops listening. */
    subscribe: (listener: MessageListener) => () => void;
};

const CommunityFeedContext = createContext<CommunityFeedValue>({
    status: 'connecting',
    roster: EMPTY_ROSTER,
    subscribe: () => () => undefined,
});

/** The community feed of the server on screen; available below `CommunityFeedProvider`. */
export const useCommunityFeed = (): CommunityFeedValue => useContext(CommunityFeedContext);

/** Runs the community feed: the roster, the structure refetches, the removal and the text messages. */
const useFeedConnection = (communityId: CommunityId, onRemoved: (reason: RemovedReason) => void) => {
    const queryClient = useQueryClient();
    const [status, setStatus] = useState<FeedStatus>('connecting');
    const [roster, setRoster] = useState<VoiceRoster>(EMPTY_ROSTER);
    const listeners = useRef(new Set<MessageListener>());
    const onRemovedRef = useRef(onRemoved);
    useEffect(() => {
        onRemovedRef.current = onRemoved;
    });

    useEffect(() => {
        const refetch = () => {
            void queryClient.invalidateQueries({ queryKey: queryKeys.communities.detail(communityId) });
            void queryClient.invalidateQueries({ queryKey: queryKeys.communities.members(communityId) });
        };
        const onMessage = (message: FeedMessage) => {
            setRoster((current) => applyToRoster(current, message));
            // Role changes come as `structure_changed`, which refetches the members too.
            if (message.channel === 'structure_changed') refetch();
            if (message.channel === 'members_changed') {
                void queryClient.invalidateQueries({ queryKey: queryKeys.communities.members(communityId) });
            }
            if (message.channel === 'removed') onRemovedRef.current(message.reason);
            if (message.channel !== 'message') return;
            for (const listener of listeners.current) listener(message.channelId, message.message);
        };
        setRoster(EMPTY_ROSTER);
        const connection = createFeedConnection({
            communityId,
            onStatus: (next) => {
                setStatus(next);
                // Whatever changed while the feed was down is in the view now.
                if (next === 'open') refetch();
            },
            onMessage,
            checkMembership: async () => {
                try {
                    await queryClient.fetchQuery({ ...communityQueryOptions(communityId), staleTime: 0 });
                    return null;
                } catch (error) {
                    return error instanceof CommunityRequestError ? error.failure : 'failed';
                }
            },
            onRemoved: (reason) => onRemovedRef.current(reason),
        });
        return connection.dispose;
    }, [communityId, queryClient]);

    const subscribe = useCallback((listener: MessageListener) => {
        listeners.current.add(listener);
        return () => {
            listeners.current.delete(listener);
        };
    }, []);

    return { status, roster, subscribe };
};

type CommunityFeedProviderProps = {
    communityId: CommunityId;
    /** The user is no longer in this community (kicked, left, or it was deleted). */
    onRemoved: (reason: RemovedReason) => void;
    children: ReactNode;
};

export const CommunityFeedProvider = ({ communityId, onRemoved, children }: CommunityFeedProviderProps) => {
    const value = useFeedConnection(communityId, onRemoved);
    return <CommunityFeedContext.Provider value={value}>{children}</CommunityFeedContext.Provider>;
};
