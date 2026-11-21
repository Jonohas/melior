import type { CategoryId, ChannelKind, PermissionOverride } from './community';
import type { MediaMode } from './media';
import { type OverrideBody, toOverrideBody } from './overrides';

export type NewChannel = {
    name: string;
    kind: ChannelKind;
    categoryId: CategoryId | null;
    topic?: string;
    mediaMode?: MediaMode;
    userLimit?: number | null;
};

/** The request body of `POST /api/communities/{id}/channels`; unset fields are left to the server. */
export const toNewChannelBody = (channel: NewChannel) => ({
    name: channel.name,
    kind: channel.kind,
    ...(channel.categoryId === null ? {} : { category_id: channel.categoryId }),
    ...(channel.topic === undefined ? {} : { topic: channel.topic }),
    ...(channel.mediaMode === undefined ? {} : { media_mode: channel.mediaMode }),
    ...(channel.userLimit === undefined || channel.userLimit === null ? {} : { user_limit: channel.userLimit }),
});

export type ChannelChange = {
    name?: string;
    topic?: string;
    /** null takes the channel out of its category. */
    categoryId?: CategoryId | null;
    position?: number;
    mediaMode?: MediaMode;
    /** null removes the limit. */
    userLimit?: number | null;
    overrides?: PermissionOverride[];
    synced?: boolean;
};

type ChannelPatchBody = {
    name?: string;
    topic?: string;
    category_id?: CategoryId | null;
    position?: number;
    media_mode?: MediaMode;
    user_limit?: number | null;
    overrides?: OverrideBody[];
    synced?: boolean;
};

/** The request body of `PATCH /api/channels/{id}`: only the fields that change, `null` where one is cleared. */
export const toChannelPatchBody = (change: ChannelChange): ChannelPatchBody => ({
    ...(change.name === undefined ? {} : { name: change.name }),
    ...(change.topic === undefined ? {} : { topic: change.topic }),
    ...(change.categoryId === undefined ? {} : { category_id: change.categoryId }),
    ...(change.position === undefined ? {} : { position: change.position }),
    ...(change.mediaMode === undefined ? {} : { media_mode: change.mediaMode }),
    ...(change.userLimit === undefined ? {} : { user_limit: change.userLimit }),
    ...(change.overrides === undefined ? {} : { overrides: change.overrides.map(toOverrideBody) }),
    ...(change.synced === undefined ? {} : { synced: change.synced }),
});

export type CategoryChange = { name?: string; position?: number; overrides?: PermissionOverride[] };

/** The request body of `PATCH /api/communities/{id}/categories/{category_id}`. */
export const toCategoryPatchBody = (change: CategoryChange) => ({
    ...(change.name === undefined ? {} : { name: change.name }),
    ...(change.position === undefined ? {} : { position: change.position }),
    ...(change.overrides === undefined ? {} : { overrides: change.overrides.map(toOverrideBody) }),
});
