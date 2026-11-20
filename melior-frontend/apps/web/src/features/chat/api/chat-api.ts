import { type ChatMessage, type HistoryPage, parseChatMessage, parseHistoryPage } from '@/features/chat/domain';
import { ForbiddenError, UnauthorizedError } from '@/lib/auth';

/** How many messages one request asks for. */
export const HISTORY_PAGE_SIZE = 50;

const messagesUrl = (channelId: string): string => `/api/channels/${encodeURIComponent(channelId)}/messages`;

const historyUrl = (channelId: string, before: number | null): string => {
    const query = new URLSearchParams({ limit: String(HISTORY_PAGE_SIZE) });
    if (before !== null) query.set('before', String(before));
    return `${messagesUrl(channelId)}?${query}`;
};

/** The plain-text reason of a refusal (400, 403, 409, 429), when the server gave a short one. */
const refusalText = async (response: Response): Promise<string | null> => {
    const text = (await response.text().catch(() => '')).trim();
    return text !== '' && text.length < 200 ? text : null;
};

/**
 * One page of a channel's chat history, oldest message first: the newest page, or with `before` the messages
 * before that id. 404 means the channel does not exist (any more).
 */
export const fetchHistory = async (channelId: string, before: number | null = null): Promise<HistoryPage> => {
    const response = await fetch(historyUrl(channelId, before));
    if (response.status === 401) throw new UnauthorizedError();
    if (response.status === 403) throw new ForbiddenError('You may not read this channel.');
    if (response.status === 404) throw new Error('This channel does not exist.');
    if (!response.ok) throw new Error(`Chat history request failed with HTTP ${response.status}`);
    const page = parseHistoryPage(await response.json());
    if (!page) throw new Error('Unexpected chat history response');
    return page;
};

/** Posts a message to a text channel and returns it as saved. Rejects with the server's reason when refused. */
export const postChatMessage = async (channelId: string, text: string, clientId: string): Promise<ChatMessage> => {
    const response = await fetch(messagesUrl(channelId), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ text, client_id: clientId }),
    });
    if (response.status === 401) throw new UnauthorizedError();
    if (!response.ok) throw new Error((await refusalText(response)) ?? `Not sent (HTTP ${response.status}).`);
    const message = parseChatMessage(await response.json());
    if (!message) throw new Error('Unexpected chat message response');
    return message;
};
