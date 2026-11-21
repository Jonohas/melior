import type { CommunityView, Role } from './community';

/** Wire shapes and parsed views shared by the domain tests. */
export const COMMUNITY_ID = '11111111-1111-4111-8111-111111111111';

export const wireChannel = {
    id: 'c-voice',
    community_id: COMMUNITY_ID,
    name: 'General',
    topic: '',
    kind: 'voice',
    category_id: 'cat-1',
    position: 0,
    synced: true,
    overrides: [],
    media_mode: 'peer_to_peer',
    user_limit: 4,
    effective_mode: 'peer_to_peer',
    effective_limit: 4,
    media_warning: null,
    my_permissions: ['view_channel', 'connect'],
};

export const wireView = {
    id: COMMUNITY_ID,
    name: 'Melior',
    owner_id: 'u-owner',
    owner_username: 'alice',
    me: {
        user_id: 'u-me',
        role_ids: ['r-mod'],
        permissions: ['view_channel', 'manage_roles'],
        rank: 5,
        is_owner: false,
    },
    roles: [
        { id: COMMUNITY_ID, name: '@everyone', color: null, position: 0, permissions: ['view_channel'] },
        { id: 'r-mod', name: 'Mod', color: 0xff0000, position: 5, permissions: ['manage_roles'] },
    ],
    categories: [{ id: 'cat-1', name: 'Voice channels', position: 1, overrides: [] }],
    channels: [wireChannel],
};

export const role = (id: string, position: number): Role => ({ id, name: id, color: null, position, permissions: [] });

export const view = (change: Partial<CommunityView> = {}): CommunityView => ({
    id: COMMUNITY_ID,
    name: 'Melior',
    ownerId: 'u-owner',
    ownerUsername: 'alice',
    me: { userId: 'u-me', roleIds: ['r-mod'], permissions: ['manage_roles', 'kick_members'], rank: 5, isOwner: false },
    roles: [role(COMMUNITY_ID, 0), role('r-mod', 5), role('r-low', 2), role('r-high', 8)],
    categories: [],
    channels: [],
    ...change,
});
