import { describe, expect, test } from 'vitest';
import { buildChannelTree, findChannel, firstChannel } from './channel-tree';
import type { Category, Channel } from './community';

const channel = (id: string, categoryId: string | null, position: number, kind = 'text') =>
    ({ id, name: id, categoryId, position, kind }) as unknown as Channel;
const category = (id: string, position: number): Category => ({ id, name: id, position, overrides: [] });

const view = {
    categories: [category('voice', 2), category('text', 1), category('empty', 3)],
    channels: [channel('b', 'text', 1), channel('a', 'text', 0), channel('v', 'voice', 0), channel('loose', null, 0)],
};

describe('buildChannelTree', () => {
    test('loose channels first, then categories and channels by position', () => {
        const tree = buildChannelTree(view, false);
        expect(tree.map((group) => group.category?.id ?? null)).toEqual([null, 'text', 'voice']);
        expect(tree[1]?.channels.map((c) => c.id)).toEqual(['a', 'b']);
    });

    test('shows empty categories only when asked', () => {
        expect(buildChannelTree(view, true).map((group) => group.category?.id ?? null)).toEqual([
            null,
            'text',
            'voice',
            'empty',
        ]);
    });

    test('a channel of an unknown category counts as loose', () => {
        const tree = buildChannelTree({ categories: [], channels: [channel('x', 'gone', 0)] }, false);
        expect(tree).toEqual([{ category: null, channels: [channel('x', 'gone', 0)] }]);
    });
});

describe('firstChannel and findChannel', () => {
    test('the first text channel in display order, else the first channel', () => {
        expect(firstChannel(view)?.id).toBe('loose');
        expect(firstChannel(view, 'loose')?.id).toBe('a');
        const voiceOnly = {
            categories: [],
            channels: [channel('v1', null, 0, 'voice'), channel('v2', null, 1, 'voice')],
        };
        expect(firstChannel(voiceOnly)?.id).toBe('v1');
        expect(firstChannel(voiceOnly, 'v1')?.id).toBe('v2');
        expect(firstChannel({ ...view, channels: [channel('v', null, 0, 'voice'), channel('t', 'text', 0)] })?.id).toBe(
            't',
        );
        expect(firstChannel({ categories: [], channels: [] })).toBeNull();
        expect(findChannel(view, 'v')?.id).toBe('v');
        expect(findChannel(view, 'zz')).toBeNull();
    });
});
