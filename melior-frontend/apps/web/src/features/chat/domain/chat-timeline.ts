import type { OutgoingAttachment } from './chat-attachment';
import type { ChatMessage } from './chat-message';
import type { PendingMessage } from './chat-state';

/** Messages by the same person closer together than this share one header. */
export const GROUP_WINDOW_MS = 5 * 60 * 1000;

export type TimelineItem =
    | { kind: 'day'; key: string; label: string }
    | {
          kind: 'message';
          key: string;
          username: string;
          /** Empty when the message is only an attachment. */
          text: string;
          /** A saved image's content type is not needed to show it, so a pending image looks the same. */
          attachment: OutgoingAttachment | null;
          /** Null for a message that is not saved yet. */
          sentAtMs: number | null;
          status: 'saved' | 'sending' | 'failed';
          error: string | null;
          /** Set for a message this client sent and the server has not echoed back. */
          clientId: string | null;
          /** False for a message that continues the one before it. */
          showHeader: boolean;
      };

const startOfDay = (date: Date): number => new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();

const DAY_MS = 24 * 60 * 60 * 1000;

/** "Today", "Yesterday" or the date, in the reader's time zone. */
export const dayLabel = (sentAtMs: number, now: Date, locale?: string): string => {
    const sent = new Date(sentAtMs);
    const daysAgo = Math.round((startOfDay(now) - startOfDay(sent)) / DAY_MS);
    if (daysAgo === 0) return 'Today';
    if (daysAgo === 1) return 'Yesterday';
    return sent.toLocaleDateString(locale, { weekday: 'short', day: 'numeric', month: 'long' });
};

const sameDay = (a: number, b: number): boolean => startOfDay(new Date(a)) === startOfDay(new Date(b));

/**
 * The rows of the chat panel: saved messages with a divider for each day, then the messages still
 * on their way. Consecutive messages by one person only show the header once.
 */
export const buildTimeline = (
    saved: readonly ChatMessage[],
    pending: readonly PendingMessage[],
    now: Date,
    locale?: string,
): TimelineItem[] => {
    const items: TimelineItem[] = [];
    let previous: { username: string; sentAtMs: number } | null = null;

    for (const message of saved) {
        const newDay = previous === null || !sameDay(previous.sentAtMs, message.sentAtMs);
        if (newDay)
            items.push({ kind: 'day', key: `day-${message.id}`, label: dayLabel(message.sentAtMs, now, locale) });
        const continues =
            previous !== null &&
            !newDay &&
            previous.username === message.username &&
            message.sentAtMs - previous.sentAtMs < GROUP_WINDOW_MS;
        items.push({
            kind: 'message',
            key: `m-${message.id}`,
            username: message.username,
            text: message.text,
            attachment: message.attachment,
            sentAtMs: message.sentAtMs,
            status: 'saved',
            error: null,
            clientId: null,
            showHeader: !continues,
        });
        previous = { username: message.username, sentAtMs: message.sentAtMs };
    }

    for (const entry of pending) {
        const continues = previous !== null && previous.username === entry.username;
        items.push({
            kind: 'message',
            key: `p-${entry.clientId}`,
            username: entry.username,
            text: entry.text,
            attachment: entry.attachment,
            sentAtMs: null,
            status: entry.status,
            error: entry.error,
            clientId: entry.clientId,
            showHeader: !continues,
        });
        previous = { username: entry.username, sentAtMs: previous?.sentAtMs ?? 0 };
    }
    return items;
};
