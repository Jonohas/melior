import { type HistoryPage, parseHistoryPage } from '@/features/chat/domain';
import { ForbiddenError, UnauthorizedError } from '@/lib/auth';

/** How many messages one request asks for. */
export const HISTORY_PAGE_SIZE = 50;

const historyUrl = (channelId: string, before: number | null): string => {
    const query = new URLSearchParams({ limit: String(HISTORY_PAGE_SIZE) });
    if (before !== null) query.set('before', String(before));
    return `/api/rooms/${encodeURIComponent(channelId)}/messages?${query}`;
};

/**
 * One page of a room's chat history, oldest message first: the newest page, or with `before` the messages
 * before that id. A room nobody has registered yet has no history, so a 404 is an empty page.
 */
export const fetchHistory = async (channelId: string, before: number | null = null): Promise<HistoryPage> => {
    const response = await fetch(historyUrl(channelId, before));
    if (response.status === 404) return { messages: [], hasMore: false };
    if (response.status === 401) throw new UnauthorizedError();
    if (response.status === 403) throw new ForbiddenError('You may not read this room.');
    if (!response.ok) throw new Error(`Chat history request failed with HTTP ${response.status}`);
    const page = parseHistoryPage(await response.json());
    if (!page) throw new Error('Unexpected chat history response');
    return page;
};
