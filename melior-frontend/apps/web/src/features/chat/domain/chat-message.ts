/** A message that was said in a room and saved by the server. */
export type ChatMessage = {
    /** Orders the room's messages; the server assigns it. */
    id: number;
    username: string;
    text: string;
    /** Milliseconds since the Unix epoch. */
    sentAtMs: number;
};

/** A page of a room's history, oldest message first. */
export type HistoryPage = {
    messages: ChatMessage[];
    /** Whether older messages exist than the oldest one in `messages`. */
    hasMore: boolean;
};

const isObject = (value: unknown): value is Record<string, unknown> =>
    typeof value === 'object' && value !== null && !Array.isArray(value);

/** Maps a saved message as the server sends it (a live `chat` message or a history entry). Null if it does not fit. */
export const parseChatMessage = (raw: unknown): ChatMessage | null => {
    if (!isObject(raw)) return null;
    const { id, username, text, sent_at_ms } = raw;
    if (typeof id !== 'number' || !Number.isSafeInteger(id)) return null;
    if (typeof sent_at_ms !== 'number' || !Number.isSafeInteger(sent_at_ms)) return null;
    if (typeof username !== 'string' || typeof text !== 'string') return null;
    return { id, username, text, sentAtMs: sent_at_ms };
};

/** Validates a `GET /api/channels/{id}/messages` response; null when it is not the documented shape. */
export const parseHistoryPage = (raw: unknown): HistoryPage | null => {
    if (!isObject(raw) || !Array.isArray(raw.messages) || typeof raw.has_more !== 'boolean') return null;
    const messages = raw.messages.map(parseChatMessage);
    if (!messages.every((message): message is ChatMessage => message !== null)) return null;
    return { messages, hasMore: raw.has_more };
};

/** Merges two lists of saved messages into one, oldest first, with each message once. */
export const mergeMessages = (current: readonly ChatMessage[], incoming: readonly ChatMessage[]): ChatMessage[] => {
    const byId = new Map<number, ChatMessage>();
    for (const message of [...current, ...incoming]) byId.set(message.id, message);
    return [...byId.values()].sort((a, b) => a.id - b.id);
};
