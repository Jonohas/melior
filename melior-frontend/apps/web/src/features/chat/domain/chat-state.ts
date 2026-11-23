import type { OutgoingAttachment } from './chat-attachment';
import { type ChatMessage, mergeMessages } from './chat-message';

/** A message this client sent that the server has not saved and echoed back yet. */
export type PendingMessage = {
    /** Chosen by this client; the server echoes it on the author's copy of the saved message. */
    clientId: string;
    username: string;
    text: string;
    /** What it carries besides the text; an image is uploaded already, so a retry sends the same id. */
    attachment: OutgoingAttachment | null;
    status: 'sending' | 'failed';
    /** Why it failed, in words fit for the person who sent it. */
    error: string | null;
};

export type HistoryStatus = 'idle' | 'loading' | 'loaded' | 'error';

export type ChatState = {
    /** Saved messages, oldest first. */
    saved: ChatMessage[];
    pending: PendingMessage[];
    /** Whether older messages exist than the oldest saved one. */
    hasMore: boolean;
    history: HistoryStatus;
    loadingEarlier: boolean;
    /** Messages that arrived while the panel was closed. */
    unread: number;
    /** A refusal that names no message of ours, to show once. */
    notice: string | null;
};

export type ChatAction =
    | { type: 'reset' }
    | { type: 'history_loading' }
    | { type: 'history_failed' }
    | { type: 'newest_loaded'; messages: ChatMessage[]; hasMore: boolean }
    | { type: 'earlier_loading' }
    | { type: 'earlier_failed' }
    | { type: 'earlier_loaded'; messages: ChatMessage[]; hasMore: boolean }
    | { type: 'received'; message: ChatMessage; clientId: string | null; countUnread: boolean }
    | {
          type: 'send_started';
          clientId: string;
          username: string;
          text: string;
          attachment: OutgoingAttachment | null;
      }
    | { type: 'send_retried'; clientId: string }
    | { type: 'send_failed'; clientId: string; error: string }
    | { type: 'refused'; clientId: string | null; message: string }
    | { type: 'notice_cleared' }
    | { type: 'panel_opened' };

export const createInitialChatState = (): ChatState => ({
    saved: [],
    pending: [],
    hasMore: false,
    history: 'idle',
    loadingEarlier: false,
    unread: 0,
    notice: null,
});

const updatePending = (
    pending: PendingMessage[],
    clientId: string,
    change: Partial<PendingMessage>,
): PendingMessage[] => pending.map((entry) => (entry.clientId === clientId ? { ...entry, ...change } : entry));

export const chatReducer = (state: ChatState, action: ChatAction): ChatState => {
    switch (action.type) {
        case 'reset':
            return createInitialChatState();
        case 'history_loading':
            // A reload after a reconnect keeps showing what is there instead of flashing a loading state.
            return state.history === 'loaded' ? state : { ...state, history: 'loading' };
        case 'history_failed':
            return { ...state, history: 'error' };
        case 'newest_loaded':
            return {
                ...state,
                saved: mergeMessages(state.saved, action.messages),
                // Only the first load says whether there is more: a reload after a reconnect sees the same tail.
                hasMore: state.history === 'loaded' ? state.hasMore : action.hasMore,
                history: 'loaded',
            };
        case 'earlier_loading':
            return { ...state, loadingEarlier: true };
        case 'earlier_failed':
            return { ...state, loadingEarlier: false, notice: 'Could not load earlier messages.' };
        case 'earlier_loaded':
            return {
                ...state,
                saved: mergeMessages(state.saved, action.messages),
                hasMore: action.hasMore,
                loadingEarlier: false,
            };
        case 'received':
            return {
                ...state,
                saved: mergeMessages(state.saved, [action.message]),
                pending: state.pending.filter((entry) => entry.clientId !== action.clientId),
                unread: action.countUnread ? state.unread + 1 : state.unread,
            };
        case 'send_started':
            return {
                ...state,
                pending: [
                    ...state.pending,
                    {
                        clientId: action.clientId,
                        username: action.username,
                        text: action.text,
                        attachment: action.attachment,
                        status: 'sending',
                        error: null,
                    },
                ],
            };
        case 'send_retried':
            return {
                ...state,
                pending: updatePending(state.pending, action.clientId, { status: 'sending', error: null }),
            };
        case 'send_failed':
            return {
                ...state,
                pending: updatePending(state.pending, action.clientId, { status: 'failed', error: action.error }),
            };
        case 'refused': {
            const ours = action.clientId !== null && state.pending.some((entry) => entry.clientId === action.clientId);
            if (!ours) return { ...state, notice: action.message };
            return {
                ...state,
                pending: updatePending(state.pending, action.clientId as string, {
                    status: 'failed',
                    error: action.message,
                }),
            };
        }
        case 'notice_cleared':
            return { ...state, notice: null };
        case 'panel_opened':
            return { ...state, unread: 0 };
    }
};
