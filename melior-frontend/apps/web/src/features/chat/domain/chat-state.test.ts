import { describe, expect, test } from 'vitest';
import type { ChatMessage } from './chat-message';
import { type ChatAction, type ChatState, chatReducer, createInitialChatState } from './chat-state';

const message = (id: number, text = `m${id}`, username = 'alice'): ChatMessage => ({
    id,
    username,
    text,
    sentAtMs: 1_700_000_000_000 + id,
    attachments: [],
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
            { type: 'send_started', clientId: 'c1', username: 'alice', text: 'hi', attachments: [] },
            { type: 'send_started', clientId: 'c2', username: 'alice', text: 'there', attachments: [] },
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
    const sending = run([{ type: 'send_started', clientId: 'c1', username: 'alice', text: 'hi', attachments: [] }]);

    test('a started send is pending', () => {
        expect(sending.pending).toEqual([
            { clientId: 'c1', username: 'alice', text: 'hi', attachments: [], status: 'sending', error: null },
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

    test('a message with an attachment and no text is pending with its attachment, and settles like any other', () => {
        const gif = { kind: 'gif' as const, url: 'https://static.klipy.com/a.gif', width: 200, height: 100 };
        const started = run([
            { type: 'send_started', clientId: 'g1', username: 'alice', text: '', attachments: [gif] },
        ]);
        expect(started.pending[0]).toMatchObject({ text: '', attachments: [gif], status: 'sending' });
        const failed = chatReducer(started, { type: 'refused', clientId: 'g1', message: 'too fast' });
        expect(failed.pending[0]).toMatchObject({ attachments: [gif], status: 'failed', error: 'too fast' });
        const savedGif = { ...message(10, ''), attachments: [gif] };
        const settled = chatReducer(failed, {
            type: 'received',
            message: savedGif,
            clientId: 'g1',
            countUnread: false,
        });
        expect(settled.pending).toEqual([]);
        expect(settled.saved).toEqual([savedGif]);
    });

    test('a message with several images keeps them in order while pending', () => {
        const images = [
            { kind: 'image' as const, id: 'i1' },
            { kind: 'image' as const, id: 'i2' },
        ];
        const started = run([
            { type: 'send_started', clientId: 'm1', username: 'alice', text: 'look', attachments: images },
        ]);
        expect(started.pending[0]?.attachments).toEqual(images);
        const retried = chatReducer(chatReducer(started, { type: 'refused', clientId: 'm1', message: 'x' }), {
            type: 'send_retried',
            clientId: 'm1',
        });
        expect(retried.pending[0]).toMatchObject({ attachments: images, status: 'sending' });
    });

    test('reset forgets everything', () => {
        expect(chatReducer(sending, { type: 'reset' })).toEqual(createInitialChatState());
    });
});
