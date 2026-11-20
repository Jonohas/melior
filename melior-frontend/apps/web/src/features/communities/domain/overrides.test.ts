import { describe, expect, test } from 'vitest';
import type { Category, Channel, PermissionOverride } from './community';
import { addTarget, choiceOf, effectiveOverrides, removeTarget, setChoice, toOverrideBody } from './overrides';

const role = { kind: 'role', id: 'r1' } as const;
const member = { kind: 'member', id: 'u1' } as const;
const base: PermissionOverride[] = [{ target: role, allow: ['connect'], deny: ['send_messages'] }];

describe('choiceOf', () => {
    test('reads allow, deny and inherit', () => {
        expect(choiceOf(base[0], 'connect')).toBe('allow');
        expect(choiceOf(base[0], 'send_messages')).toBe('deny');
        expect(choiceOf(base[0], 'view_channel')).toBe('inherit');
        expect(choiceOf(undefined, 'connect')).toBe('inherit');
    });
});

describe('setChoice', () => {
    test('moves a bit between allow, deny and inherit', () => {
        expect(setChoice(base, role, 'connect', 'deny')).toEqual([
            { target: role, allow: [], deny: ['connect', 'send_messages'] },
        ]);
        expect(setChoice(base, role, 'send_messages', 'inherit')).toEqual([
            { target: role, allow: ['connect'], deny: [] },
        ]);
    });

    test('creates an override for a new target', () => {
        expect(setChoice(base, member, 'view_channel', 'allow')).toEqual([
            ...base,
            { target: member, allow: ['view_channel'], deny: [] },
        ]);
    });
});

describe('targets', () => {
    test('adds once and removes', () => {
        expect(addTarget(base, role)).toEqual(base);
        expect(addTarget(base, member)).toHaveLength(2);
        expect(removeTarget(base, role)).toEqual([]);
    });

    test('toOverrideBody keeps the wire shape', () => {
        expect(toOverrideBody(base[0] as PermissionOverride)).toEqual({
            target: { kind: 'role', id: 'r1' },
            allow: ['connect'],
            deny: ['send_messages'],
        });
    });
});

describe('effectiveOverrides', () => {
    const category: Category = { id: 'cat', name: 'C', position: 0, overrides: base };
    const channel = { categoryId: 'cat', synced: true, overrides: [] } as unknown as Channel;

    test("a synced channel in a category uses the category's overrides", () => {
        expect(effectiveOverrides(channel, [category])).toBe(base);
        expect(effectiveOverrides({ ...channel, synced: false }, [category])).toEqual([]);
        expect(effectiveOverrides({ ...channel, categoryId: null }, [category])).toEqual([]);
    });
});
