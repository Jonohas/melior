import { describe, expect, test } from 'vitest';
import type { ChatMessage } from './chat-message';
import { type ChatAction, type ChatState, chatReducer, createInitialChatState } from './chat-state';

const message = (id: number, text = `m${id}`, username = 'alice'): ChatMessage => ({
    id,
    username,
    text,
    sentAtMs: 1_700_000_000_000 + id,
});

const run = (actions: ChatAction[], from: ChatState = createInitialChatState()) => actions.reduce(chatReducer, from);

describe('chatReducer history', () => {
    test('the first newest page sets the messages and whether there is more', () => {
        const state = run([
            { type: 'history_loading' },
            { type: 'newest_loaded', messages: [message(4), message(5)], hasMore: true },
        ]);
        expect(state.saved.map((m) => m.id)).toEqual([4, 5]);
        expect(state.hasMore).toBe(true);
        expect(state.history).toBe('loaded');
    });

    test('a reload after a reconnect merges what was missed and keeps the earlier paging state', () => {
        const state = run([
            { type: 'newest_loaded', messages: [message(4), message(5)], hasMore: true },
            { type: 'earlier_loaded', messages: [message(1), message(2), message(3)], hasMore: false },
            { type: 'history_loading' },
            { type: 'newest_loaded', messages: [message(5), message(6), message(7)], hasMore: true },
        ]);
        expect(state.saved.map((m) => m.id)).toEqual([1, 2, 3, 4, 5, 6, 7]);
        expect(state.hasMore).toBe(false);
    });

    test('an earlier page goes in front and ends the paging when nothing older is left', () => {
        const state = run([
            { type: 'newest_loaded', messages: [message(3)], hasMore: true },
            { type: 'earlier_loading' },
            { type: 'earlier_loaded', messages: [message(1), message(2)], hasMore: false },
        ]);
        expect(state.saved.map((m) => m.id)).toEqual([1, 2, 3]);
        expect(state.hasMore).toBe(false);
        expect(state.loadingEarlier).toBe(false);
    });

    test('a failed page of earlier messages stops loading, keeps what is there and says so', () => {
        const state = run([
            { type: 'newest_loaded', messages: [message(3)], hasMore: true },
            { type: 'earlier_loading' },
            { type: 'earlier_failed' },
        ]);
        expect(state.loadingEarlier).toBe(false);
        expect(state.hasMore).toBe(true);
        expect(state.saved.map((m) => m.id)).toEqual([3]);
        expect(state.notice).toBe('Could not load earlier messages.');
    });

    test('a failed load is reported and a later load clears it', () => {
        const failed = run([{ type: 'history_loading' }, { type: 'history_failed' }]);
        expect(failed.history).toBe('error');
        expect(chatReducer(failed, { type: 'newest_loaded', messages: [], hasMore: false }).history).toBe('loaded');
    });
});

describe('chatReducer live messages', () => {
    test('a message is saved once however often it arrives, in id order', () => {
        const state = run([
            { type: 'received', message: message(2), clientId: null, countUnread: false },
            { type: 'received', message: message(1), clientId: null, countUnread: false },
            { type: 'received', message: message(2), clientId: null, countUnread: false },
        ]);
        expect(state.saved.map((m) => m.id)).toEqual([1, 2]);
    });

    test('the author copy replaces the pending message with the same client id', () => {
        const state = run([
            { type: 'send_started', clientId: 'c1', username: 'alice', text: 'hi' },
            { type: 'send_started', clientId: 'c2', username: 'alice', text: 'there' },
            { type: 'received', message: message(9, 'hi'), clientId: 'c1', countUnread: false },
        ]);
        expect(state.saved).toEqual([message(9, 'hi')]);
        expect(state.pending.map((p) => p.clientId)).toEqual(['c2']);
    });

    test('only messages that arrive while the panel is closed count as unread, until it opens', () => {
        const state = run([
            { type: 'received', message: message(1), clientId: null, countUnread: true },
            { type: 'received', message: message(2), clientId: null, countUnread: true },
            { type: 'received', message: message(3), clientId: null, countUnread: false },
        ]);
        expect(state.unread).toBe(2);
        expect(chatReducer(state, { type: 'panel_opened' }).unread).toBe(0);
    });
});

describe('chatReducer sending', () => {
    const sending = run([{ type: 'send_started', clientId: 'c1', username: 'alice', text: 'hi' }]);

    test('a started send is pending', () => {
        expect(sending.pending).toEqual([
            { clientId: 'c1', username: 'alice', text: 'hi', status: 'sending', error: null },
        ]);
    });

    test('a refusal that names the message fails it with the reason, and a retry makes it pending again', () => {
        const refused = chatReducer(sending, { type: 'refused', clientId: 'c1', message: 'too fast' });
        expect(refused.pending[0]).toMatchObject({ status: 'failed', error: 'too fast' });
        expect(refused.notice).toBeNull();
        const retried = chatReducer(refused, { type: 'send_retried', clientId: 'c1' });
        expect(retried.pending[0]).toMatchObject({ status: 'sending', error: null });
    });

    test('a refusal that names no message of ours becomes a notice that can be cleared', () => {
        const unnamed = chatReducer(sending, { type: 'refused', clientId: null, message: 'try again' });
        expect(unnamed.notice).toBe('try again');
        expect(unnamed.pending[0]?.status).toBe('sending');
        const unknown = chatReducer(sending, { type: 'refused', clientId: 'other', message: 'try again' });
        expect(unknown.notice).toBe('try again');
        expect(chatReducer(unknown, { type: 'notice_cleared' }).notice).toBeNull();
    });

    test('a send that never left fails with its own reason', () => {
        const state = chatReducer(sending, { type: 'send_failed', clientId: 'c1', error: 'Not connected' });
        expect(state.pending[0]).toMatchObject({ status: 'failed', error: 'Not connected' });
    });

    test('reset forgets everything', () => {
        expect(chatReducer(sending, { type: 'reset' })).toEqual(createInitialChatState());
    });
});
