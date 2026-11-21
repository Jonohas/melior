import { describe, expect, test } from 'vitest';
import { parseChannel, parseCommunitySummaries, parseCommunityView, parseOverride } from './community';
import { COMMUNITY_ID, wireChannel, wireView } from './test-fixtures';

describe('parseCommunityView', () => {
    test('maps the wire shape', () => {
        const parsed = parseCommunityView(wireView);
        expect(parsed?.me).toEqual({
            userId: 'u-me',
            roleIds: ['r-mod'],
            permissions: ['view_channel', 'manage_roles'],
            rank: 5,
            isOwner: false,
        });
        expect(parsed?.roles[1]).toEqual({
            id: 'r-mod',
            name: 'Mod',
            color: 0xff0000,
            position: 5,
            permissions: ['manage_roles'],
        });
        expect(parsed?.channels[0]).toEqual({
            id: 'c-voice',
            communityId: COMMUNITY_ID,
            name: 'General',
            topic: '',
            kind: 'voice',
            categoryId: 'cat-1',
            position: 0,
            synced: true,
            overrides: [],
            mediaMode: 'peer_to_peer',
            userLimit: 4,
            effectiveMode: 'peer_to_peer',
            effectiveLimit: 4,
            mediaWarning: null,
            myPermissions: ['view_channel', 'connect'],
        });
    });

    test('rejects a view with a missing or malformed part', () => {
        expect(parseCommunityView(null)).toBeNull();
        expect(parseCommunityView({ ...wireView, me: undefined })).toBeNull();
        expect(parseCommunityView({ ...wireView, me: { ...wireView.me, rank: -1 } })).toBeNull();
        // The owner's rank is u32::MAX on the wire.
        expect(parseCommunityView({ ...wireView, me: { ...wireView.me, rank: 4294967295 } })?.me.rank).toBe(4294967295);
        expect(parseCommunityView({ ...wireView, roles: 'x' })).toBeNull();
        expect(parseCommunityView({ ...wireView, channels: [{ ...wireChannel, kind: 'stage' }] })).toBeNull();
        expect(parseCommunityView({ ...wireView, owner_username: 3 })).toBeNull();
    });
});

describe('parseChannel', () => {
    test('accepts a text channel without category and defaults a missing topic', () => {
        const text = {
            ...wireChannel,
            kind: 'text',
            category_id: null,
            topic: undefined,
            media_mode: 'auto',
            user_limit: null,
            effective_mode: 'auto',
            effective_limit: null,
        };
        expect(parseChannel(text)).toMatchObject({ kind: 'text', categoryId: null, topic: '', userLimit: null });
    });

    test('rejects unknown media modes, warnings and bad ids', () => {
        expect(parseChannel({ ...wireChannel, media_mode: 'mesh' })).toBeNull();
        expect(parseChannel({ ...wireChannel, media_warning: 'tired' })).toBeNull();
        expect(parseChannel({ ...wireChannel, media_warning: 'mesh_off' })?.mediaWarning).toBe('mesh_off');
        expect(parseChannel({ ...wireChannel, id: 'a/b' })).toBeNull();
        expect(parseChannel({ ...wireChannel, user_limit: 2.5 })).toBeNull();
        expect(parseChannel({ ...wireChannel, synced: 'yes' })).toBeNull();
    });
});

describe('parseOverride', () => {
    test('parses role and member targets and rejects others', () => {
        const wire = { target: { kind: 'member', id: 'u1' }, allow: ['connect'], deny: ['view_channel'] };
        expect(parseOverride(wire)).toEqual({
            target: { kind: 'member', id: 'u1' },
            allow: ['connect'],
            deny: ['view_channel'],
        });
        expect(parseOverride({ ...wire, target: { kind: 'group', id: 'g' } })).toBeNull();
        expect(parseOverride({ ...wire, allow: 'connect' })).toBeNull();
    });
});

describe('parseCommunitySummaries', () => {
    test('maps the list and rejects a bad entry', () => {
        expect(parseCommunitySummaries([{ id: 'a', name: 'A', owner_username: 'bob' }])).toEqual([
            { id: 'a', name: 'A', ownerUsername: 'bob' },
        ]);
        expect(parseCommunitySummaries([{ id: 'a' }])).toBeNull();
        expect(parseCommunitySummaries({})).toBeNull();
    });
});
