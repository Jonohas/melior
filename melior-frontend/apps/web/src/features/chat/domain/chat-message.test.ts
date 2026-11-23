import { describe, expect, test } from 'vitest';
import { mergeMessages, parseChatMessage, parseHistoryPage } from './chat-message';

const wire = { id: 3, username: 'alice', text: 'hi', sent_at_ms: 1_700_000_000_000 };
const saved = { id: 3, username: 'alice', text: 'hi', sentAtMs: 1_700_000_000_000, attachment: null };

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

describe('parseChatMessage attachments', () => {
    const image = { kind: 'image', id: '0f9e', content_type: 'image/png' };
    const gif = { kind: 'gif', url: 'https://static2.klipy.com/ii/abc/1.gif', width: 498, height: 280 };

    test('maps an image and a GIF, with empty text', () => {
        expect(parseChatMessage({ ...wire, text: '', attachment: image })).toEqual({
            ...saved,
            text: '',
            attachment: { kind: 'image', id: '0f9e', contentType: 'image/png' },
        });
        expect(parseChatMessage({ ...wire, attachment: gif })).toEqual({ ...saved, attachment: gif });
    });

    test('an attachment of an unknown kind or the wrong shape makes the message invalid', () => {
        for (const attachment of [
            null,
            'image',
            [],
            { kind: 'video', id: 'x' },
            { id: '0f9e', content_type: 'image/png' },
            { ...image, id: 5 },
            { ...image, id: '' },
            { ...image, content_type: undefined },
            { ...gif, url: 'http://static.klipy.com/a.gif' },
            { ...gif, url: 'https://media.example.com/a.gif' },
            { ...gif, width: 0 },
            { ...gif, height: 4097 },
            { ...gif, width: 1.5 },
            { ...gif, height: '280' },
        ]) {
            expect(parseChatMessage({ ...wire, attachment })).toBeNull();
        }
    });

    test('a history page with one bad attachment is rejected', () => {
        expect(
            parseHistoryPage({ messages: [wire, { ...wire, id: 4, attachment: { kind: 'x' } }], has_more: false }),
        ).toBeNull();
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
