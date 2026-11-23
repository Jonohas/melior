import { describe, expect, test } from 'vitest';
import type { ChatMessage } from './chat-message';
import type { PendingMessage } from './chat-state';
import { buildTimeline, dayLabel, GROUP_WINDOW_MS } from './chat-timeline';

// Built from local calendar fields, so the tests hold in any time zone.
const at = (day: number, hour: number, minute = 0) => new Date(2026, 10, day, hour, minute).getTime();
const NOW = new Date(2026, 10, 20, 12, 0);

const message = (id: number, username: string, sentAtMs: number): ChatMessage => ({
    id,
    username,
    text: `m${id}`,
    sentAtMs,
    attachments: [],
});

const pendingOf = (
    clientId: string,
    username: string,
    status: PendingMessage['status'] = 'sending',
): PendingMessage => ({
    clientId,
    username,
    text: `p-${clientId}`,
    attachments: [],
    status,
    error: status === 'failed' ? 'too fast' : null,
});

describe('dayLabel', () => {
    test('says Today and Yesterday and otherwise gives the date', () => {
        expect(dayLabel(at(20, 8), NOW)).toBe('Today');
        expect(dayLabel(at(19, 23, 59), NOW)).toBe('Yesterday');
        expect(dayLabel(at(10, 9), NOW, 'en-GB')).toContain('10');
        expect(dayLabel(at(10, 9), NOW, 'en-GB')).not.toMatch(/Today|Yesterday/);
    });
});

describe('buildTimeline', () => {
    test('has a divider where the day changes and none inside a day', () => {
        const items = buildTimeline(
            [message(1, 'alice', at(19, 9)), message(2, 'bob', at(19, 10)), message(3, 'bob', at(20, 9))],
            [],
            NOW,
        );
        expect(items.map((i) => (i.kind === 'day' ? i.label : i.key))).toEqual([
            'Yesterday',
            'm-1',
            'm-2',
            'Today',
            'm-3',
        ]);
    });

    test('a person who keeps talking shares one header until a pause, another speaker or a new day', () => {
        const items = buildTimeline(
            [
                message(1, 'alice', at(20, 9)),
                message(2, 'alice', at(20, 9, 1)),
                message(3, 'bob', at(20, 9, 2)),
                message(4, 'bob', at(20, 9, 2) + GROUP_WINDOW_MS),
            ],
            [],
            NOW,
        );
        const headers = items.filter((i) => i.kind === 'message').map((i) => i.kind === 'message' && i.showHeader);
        expect(headers).toEqual([true, false, true, true]);
    });

    test('pending messages come last, marked with their status and client id', () => {
        const items = buildTimeline(
            [message(1, 'alice', at(20, 9))],
            [pendingOf('c1', 'alice'), pendingOf('c2', 'alice', 'failed')],
            NOW,
        );
        const rows = items.filter((i) => i.kind === 'message');
        expect(rows.map((r) => r.kind === 'message' && [r.status, r.clientId, r.showHeader, r.error])).toEqual([
            ['saved', null, true, null],
            ['sending', 'c1', false, null],
            ['failed', 'c2', false, 'too fast'],
        ]);
    });

    test('an empty room has an empty timeline', () => {
        expect(buildTimeline([], [], NOW)).toEqual([]);
    });
});
