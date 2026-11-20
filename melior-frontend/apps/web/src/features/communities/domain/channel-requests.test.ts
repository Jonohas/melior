import { describe, expect, test } from 'vitest';
import { toCategoryPatchBody, toChannelPatchBody, toNewChannelBody } from './channel-requests';

const override = { target: { kind: 'role', id: 'r' }, allow: ['connect'], deny: [] } as const;

describe('toNewChannelBody', () => {
    test('maps to snake case and leaves unset fields out', () => {
        expect(toNewChannelBody({ name: 'general', kind: 'text', categoryId: null })).toEqual({
            name: 'general',
            kind: 'text',
        });
        expect(
            toNewChannelBody({ name: 'v', kind: 'voice', categoryId: 'c', mediaMode: 'server', userLimit: 5 }),
        ).toEqual({ name: 'v', kind: 'voice', category_id: 'c', media_mode: 'server', user_limit: 5 });
    });
});

describe('toChannelPatchBody', () => {
    test('sends nulls to clear and only what changes', () => {
        expect(toChannelPatchBody({ categoryId: null, userLimit: null })).toEqual({
            category_id: null,
            user_limit: null,
        });
        expect(toChannelPatchBody({ synced: true })).toEqual({ synced: true });
        expect(toChannelPatchBody({ overrides: [{ ...override, allow: ['connect'], deny: [] }] })).toEqual({
            overrides: [{ target: { kind: 'role', id: 'r' }, allow: ['connect'], deny: [] }],
        });
    });

    test('category patches carry name, position and overrides', () => {
        expect(toCategoryPatchBody({ name: 'X', position: 2 })).toEqual({ name: 'X', position: 2 });
        expect(toCategoryPatchBody({})).toEqual({});
    });
});
