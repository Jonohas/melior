import { describe, expect, test } from 'vitest';
import { mergeMessages, parseChatMessage, parseHistoryPage } from './chat-message';

const wire = { id: 3, username: 'alice', text: 'hi', sent_at_ms: 1_700_000_000_000 };
const saved = { id: 3, username: 'alice', text: 'hi', sentAtMs: 1_700_000_000_000 };

describe('parseChatMessage', () => {
    test('maps the wire shape to a ChatMessage', () => {
        expect(parseChatMessage(wire)).toEqual(saved);
    });

    test('rejects anything that is not the documented shape', () => {
        for (const bad of [null, [], 'hi', { ...wire, id: '3' }, { ...wire, id: 1.5 }, { ...wire, text: 1 }]) {
            expect(parseChatMessage(bad)).toBeNull();
        }
        expect(parseChatMessage({ ...wire, sent_at_ms: undefined })).toBeNull();
        expect(parseChatMessage({ ...wire, username: undefined })).toBeNull();
    });
});

describe('parseHistoryPage', () => {
    test('parses a page and keeps its order', () => {
        const page = parseHistoryPage({ messages: [{ ...wire, id: 1 }, wire], has_more: true });
        expect(page?.messages.map((m) => m.id)).toEqual([1, 3]);
        expect(page?.hasMore).toBe(true);
    });

    test('rejects a page with a bad entry, a missing flag or no list', () => {
        expect(parseHistoryPage({ messages: [wire, {}], has_more: false })).toBeNull();
        expect(parseHistoryPage({ messages: [wire] })).toBeNull();
        expect(parseHistoryPage({ messages: 'none', has_more: false })).toBeNull();
        expect(parseHistoryPage([])).toBeNull();
    });
});

describe('mergeMessages', () => {
    test('orders by id and keeps each message once', () => {
        const merged = mergeMessages([saved, { ...saved, id: 5 }], [{ ...saved, id: 4 }, saved]);
        expect(merged.map((m) => m.id)).toEqual([3, 4, 5]);
    });
});
