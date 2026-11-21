import { type ChannelId, isChannelId } from '@/features/conference/domain';
import { MEDIA_MODES, MEDIA_WARNINGS, type MediaMode, type MediaWarning } from './media';
import { type Permission, parsePermissions } from './permissions';
import {
    isBoolean,
    isObject,
    isString,
    isWholeNumber,
    type JsonObject,
    nullableStringOf,
    nullableWholeOf,
    oneOf,
    parseList,
} from './wire';

/** A community ("Server" in the UI). Its id is a UUID string. */
export type CommunityId = string;
export type RoleId = string;
export type CategoryId = string;
export type UserId = string;

/** One entry of `GET /api/communities`. */
export type CommunitySummary = { id: CommunityId; name: string; ownerUsername: string };

export type Role = {
    id: RoleId;
    name: string;
    /** 0xRRGGBB, or null for the default colour. */
    color: number | null;
    /** `@everyone` is 0; higher outranks lower. */
    position: number;
    permissions: Permission[];
};

export const OVERRIDE_TARGET_KINDS = ['role', 'member'] as const;
export type OverrideTarget = { kind: (typeof OVERRIDE_TARGET_KINDS)[number]; id: string };

export type PermissionOverride = { target: OverrideTarget; allow: Permission[]; deny: Permission[] };

export type Category = { id: CategoryId; name: string; position: number; overrides: PermissionOverride[] };

export const CHANNEL_KINDS = ['voice', 'text'] as const;
export type ChannelKind = (typeof CHANNEL_KINDS)[number];

export type Channel = {
    id: ChannelId;
    communityId: CommunityId;
    name: string;
    topic: string;
    kind: ChannelKind;
    categoryId: CategoryId | null;
    position: number;
    /** Uses the category's overrides instead of its own. */
    synced: boolean;
    overrides: PermissionOverride[];
    mediaMode: MediaMode;
    userLimit: number | null;
    effectiveMode: MediaMode;
    effectiveLimit: number | null;
    mediaWarning: MediaWarning | null;
    myPermissions: Permission[];
};

/** The caller's place in the community. */
export type Me = {
    userId: UserId;
    roleIds: RoleId[];
    /** Base permissions (community wide, without channel overrides). */
    permissions: Permission[];
    /** The highest position among the caller's roles; the owner outranks everyone regardless. */
    rank: number;
    isOwner: boolean;
};

/** `GET /api/communities/{id}`: everything the caller may see of one community. */
export type CommunityView = {
    id: CommunityId;
    name: string;
    ownerId: UserId;
    ownerUsername: string;
    me: Me;
    roles: Role[];
    categories: Category[];
    /** Only the channels the caller can see, sorted by position. */
    channels: Channel[];
};

const isStringList = (value: unknown): value is string[] => Array.isArray(value) && value.every(isString);

export const parseCommunitySummary = (raw: unknown): CommunitySummary | null => {
    if (!isObject(raw) || !isString(raw.id) || !isString(raw.name) || !isString(raw.owner_username)) return null;
    return { id: raw.id, name: raw.name, ownerUsername: raw.owner_username };
};

export const parseCommunitySummaries = (raw: unknown): CommunitySummary[] | null =>
    parseList(raw, parseCommunitySummary);

export const parseRole = (raw: unknown): Role | null => {
    if (!isObject(raw) || !isString(raw.id) || !isString(raw.name) || !isWholeNumber(raw.position)) return null;
    const color = nullableWholeOf(raw.color);
    const permissions = parsePermissions(raw.permissions);
    if (color === undefined || !permissions) return null;
    return { id: raw.id, name: raw.name, color, position: raw.position, permissions };
};

export const parseOverride = (raw: unknown): PermissionOverride | null => {
    if (!isObject(raw) || !isObject(raw.target) || !isString(raw.target.id)) return null;
    const kind = oneOf(OVERRIDE_TARGET_KINDS, raw.target.kind);
    const allow = parsePermissions(raw.allow);
    const deny = parsePermissions(raw.deny);
    if (!kind || !allow || !deny) return null;
    return { target: { kind, id: raw.target.id }, allow, deny };
};

const parseOverrides = (raw: unknown): PermissionOverride[] | null => parseList(raw ?? [], parseOverride);

export const parseCategory = (raw: unknown): Category | null => {
    if (!isObject(raw) || !isString(raw.id) || !isString(raw.name) || !isWholeNumber(raw.position)) return null;
    const overrides = parseOverrides(raw.overrides);
    if (!overrides) return null;
    return { id: raw.id, name: raw.name, position: raw.position, overrides };
};

type ChannelMedia = Pick<Channel, 'mediaMode' | 'userLimit' | 'effectiveMode' | 'effectiveLimit' | 'mediaWarning'>;

const parseChannelMedia = (raw: JsonObject): ChannelMedia | null => {
    const mediaMode = oneOf(MEDIA_MODES, raw.media_mode);
    const effectiveMode = oneOf(MEDIA_MODES, raw.effective_mode ?? raw.media_mode);
    const userLimit = nullableWholeOf(raw.user_limit);
    const effectiveLimit = nullableWholeOf(raw.effective_limit);
    const warning = raw.media_warning ?? null;
    const mediaWarning = warning === null ? null : oneOf(MEDIA_WARNINGS, warning);
    if (!mediaMode || !effectiveMode || userLimit === undefined || effectiveLimit === undefined) return null;
    if (warning !== null && mediaWarning === null) return null;
    return { mediaMode, userLimit, effectiveMode, effectiveLimit, mediaWarning };
};

export const parseChannel = (raw: unknown): Channel | null => {
    if (!isObject(raw) || !isChannelId(raw.id) || !isString(raw.community_id) || !isString(raw.name)) return null;
    const kind = oneOf(CHANNEL_KINDS, raw.kind);
    const categoryId = nullableStringOf(raw.category_id);
    const topic = raw.topic ?? '';
    if (!kind || categoryId === undefined || !isString(topic)) return null;
    if (!isWholeNumber(raw.position) || !isBoolean(raw.synced)) return null;
    const overrides = parseOverrides(raw.overrides);
    const media = parseChannelMedia(raw);
    const myPermissions = parsePermissions(raw.my_permissions);
    if (!overrides || !media || !myPermissions) return null;
    return {
        id: raw.id,
        communityId: raw.community_id,
        name: raw.name,
        topic,
        kind,
        categoryId,
        position: raw.position,
        synced: raw.synced,
        overrides,
        ...media,
        myPermissions,
    };
};

const parseMe = (raw: unknown): Me | null => {
    if (!isObject(raw) || !isString(raw.user_id) || !isStringList(raw.role_ids)) return null;
    const permissions = parsePermissions(raw.permissions);
    if (!permissions || !isWholeNumber(raw.rank) || !isBoolean(raw.is_owner)) return null;
    return { userId: raw.user_id, roleIds: raw.role_ids, permissions, rank: raw.rank, isOwner: raw.is_owner };
};

export const parseCommunityView = (raw: unknown): CommunityView | null => {
    if (!isObject(raw) || !isString(raw.id) || !isString(raw.name)) return null;
    if (!isString(raw.owner_id) || !isString(raw.owner_username)) return null;
    const me = parseMe(raw.me);
    const roles = parseList(raw.roles, parseRole);
    const categories = parseList(raw.categories, parseCategory);
    const channels = parseList(raw.channels, parseChannel);
    if (!me || !roles || !categories || !channels) return null;
    return {
        id: raw.id,
        name: raw.name,
        ownerId: raw.owner_id,
        ownerUsername: raw.owner_username,
        me,
        roles,
        categories,
        channels,
    };
};
