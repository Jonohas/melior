import { useCallback, useEffect } from 'react';
import { postChatMessage } from '@/features/chat/api';
import type { DraftCheck, OutgoingAttachment } from '@/features/chat/domain';
import { type ChatTransport, type UseChatReturn, useChat } from '@/features/chat/hooks';
import { useCommunityFeed } from './community-feed-context';

export type TextChannelChat = Omit<UseChatReturn, 'submit' | 'retry'> & {
    submit: (draft: string, attachment: OutgoingAttachment | null) => DraftCheck;
    retry: (clientId: string) => void;
};

/**
 * A text channel's chat: history over HTTP, new messages from the community feed, and posts with
 * `POST /api/channels/{id}/messages`, whose answer settles the pending message (or marks it failed).
 */
export const useTextChannelChat = (channelId: string, username: string): TextChannelChat => {
    const { status, subscribe } = useCommunityFeed();
    // The newest page is (re)loaded whenever the feed (re)opens, which also catches what was missed meanwhile.
    const chat = useChat(channelId, { connected: status === 'open', username, open: true });
    const { receiveSaved, refuse } = chat;

    useEffect(
        () =>
            subscribe((messageChannel, message) => {
                if (messageChannel === channelId) receiveSaved(message, null);
            }),
        [subscribe, channelId, receiveSaved],
    );

    const transport: ChatTransport = useCallback(
        (message, clientId) => {
            postChatMessage(channelId, message, clientId)
                .then((saved) => receiveSaved(saved, clientId))
                .catch((error) => refuse(clientId, error instanceof Error ? error.message : String(error)));
            return true;
        },
        [channelId, receiveSaved, refuse],
    );

    return {
        ...chat,
        submit: (draft, attachment) => chat.submit(draft, attachment, transport),
        retry: (clientId) => chat.retry(clientId, transport),
    };
};
