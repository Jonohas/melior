import { useQueryClient } from '@tanstack/react-query';
import { useRouter } from '@tanstack/react-router';
import { useEffect, useRef, useState } from 'react';
import { type UseChatReturn, useChat } from '@/features/chat/hooks';
import { initialCallChatOpen, probeVoiceChannel } from '@/features/communities/api';
import type { ChannelId } from '@/features/conference/domain';
import { type ChatEvent, useConference } from '@/features/conference/hooks';
import { recheckSession } from '@/lib/auth';

export type VoiceCall = {
    conference: ReturnType<typeof useConference>;
    chat: UseChatReturn;
    chatOpen: boolean;
    setChatOpen: (open: boolean) => void;
    /** Why the login gate refused to redirect, if it did. */
    authNotice: string | null;
};

// Without a call there is no connection, so the chat never loads anything for this placeholder.
const NO_CHANNEL = '';

/**
 * A voice channel's call and its chat; no channel means no call. After a lost connection the conference
 * re-checks the session (401 runs the login gate, 403 shows the no-access page) and asks the channel's
 * admission endpoint whether to go on.
 */
export const useVoiceCall = (channelId: ChannelId | null, forceRelay: boolean): VoiceCall => {
    const router = useRouter();
    const queryClient = useQueryClient();
    const [authNotice, setAuthNotice] = useState<string | null>(null);
    const [chatOpen, setChatOpen] = useState(initialCallChatOpen);
    // The chat needs the connection's status and the connection needs the chat's inbox: this ref joins them.
    const chatInbox = useRef<(event: ChatEvent) => void>(() => undefined);
    const conference = useConference(channelId, {
        forceRelay,
        checkSession: async () => {
            const result = await recheckSession(queryClient, setAuthNotice);
            if (result === 'forbidden') void router.invalidate();
            return result;
        },
        checkChannelAccess: async () => (channelId === null ? 'unknown' : probeVoiceChannel(channelId)),
        onChat: (event) => chatInbox.current(event),
    });
    const chat = useChat(channelId ?? NO_CHANNEL, {
        // The status still names the old call in the render that changes the channel.
        connected: channelId !== null && conference.status === 'connected',
        username: conference.username,
        open: chatOpen,
    });
    useEffect(() => {
        chatInbox.current = chat.receive;
    }, [chat.receive]);

    return { conference, chat, chatOpen, setChatOpen, authNotice };
};
