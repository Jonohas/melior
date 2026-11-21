import type { CommunityId } from './community';
import { isBoolean, isObject, isString, isWholeNumber, nullableWholeOf, parseList } from './wire';

export type Invite = {
    code: string;
    communityId: CommunityId;
    /** Username of whoever created it. */
    createdBy: string;
    /** Milliseconds since the epoch, or null for never. */
    expiresAtMs: number | null;
    maxUses: number | null;
    uses: number;
};

/** `GET /api/invites/{code}`: what an invite leads to, before joining. */
export type InvitePreview = {
    code: string;
    community: { id: CommunityId; name: string };
    memberCount: number;
    usable: boolean;
};

export const INVITE_CODE_PATTERN = /^[A-Za-z0-9]{10}$/;

export const isInviteCode = (value: unknown): value is string =>
    typeof value === 'string' && INVITE_CODE_PATTERN.test(value);

/** A code typed or pasted by hand: the bare code, or a link that ends in `/invite/<code>`. Null otherwise. */
export const inviteCodeFromInput = (raw: string): string | null => {
    const trimmed = raw.trim().replace(/\/+$/, '');
    const candidate = trimmed.slice(trimmed.lastIndexOf('/') + 1);
    if (!isInviteCode(candidate)) return null;
    const isLink = candidate !== trimmed;
    return !isLink || trimmed.endsWith(`/invite/${candidate}`) ? candidate : null;
};

/** The link to share: same origin as this app. */
export const inviteLink = (origin: string, code: string): string => `${origin}/invite/${code}`;

/** How long a new invite stays valid; null is never. */
export const INVITE_MAX_AGES: readonly { seconds: number | null; label: string }[] = [
    { seconds: 1800, label: '30 minutes' },
    { seconds: 3600, label: '1 hour' },
    { seconds: 21600, label: '6 hours' },
    { seconds: 43200, label: '12 hours' },
    { seconds: 86400, label: '1 day' },
    { seconds: 604800, label: '7 days' },
    { seconds: null, label: 'Never' },
];

/** How often a new invite can be used; null is no limit. */
export const INVITE_MAX_USES: readonly (number | null)[] = [null, 1, 5, 10, 25, 50, 100];

export type NewInvite = { maxAgeSeconds: number | null; maxUses: number | null };

/** The request body of `POST /api/communities/{id}/invites`; "never" and "no limit" are left out. */
export const toInviteBody = (invite: NewInvite): { max_age_seconds?: number; max_uses?: number } => ({
    ...(invite.maxAgeSeconds === null ? {} : { max_age_seconds: invite.maxAgeSeconds }),
    ...(invite.maxUses === null ? {} : { max_uses: invite.maxUses }),
});

/** "expires in 3 h", "expired" or "never expires", for the invite list. */
export const describeExpiry = (expiresAtMs: number | null, nowMs: number): string => {
    if (expiresAtMs === null) return 'never expires';
    const left = expiresAtMs - nowMs;
    if (left <= 0) return 'expired';
    const minutes = Math.ceil(left / 60_000);
    if (minutes < 60) return `expires in ${minutes} min`;
    const hours = Math.round(minutes / 60);
    if (hours < 48) return `expires in ${hours} h`;
    return `expires in ${Math.round(hours / 24)} days`;
};

export const describeUses = (invite: Pick<Invite, 'uses' | 'maxUses'>): string =>
    invite.maxUses === null ? `${invite.uses} uses` : `${invite.uses} / ${invite.maxUses} uses`;

export const parseInvite = (raw: unknown): Invite | null => {
    if (!isObject(raw) || !isInviteCode(raw.code) || !isString(raw.community_id)) return null;
    const expiresAtMs = nullableWholeOf(raw.expires_at_ms);
    const maxUses = nullableWholeOf(raw.max_uses);
    if (!isString(raw.created_by) || !isWholeNumber(raw.uses)) return null;
    if (expiresAtMs === undefined || maxUses === undefined) return null;
    return {
        code: raw.code,
        communityId: raw.community_id,
        createdBy: raw.created_by,
        expiresAtMs,
        maxUses,
        uses: raw.uses,
    };
};

export const parseInvites = (raw: unknown): Invite[] | null => parseList(raw, parseInvite);

export const parseInvitePreview = (raw: unknown): InvitePreview | null => {
    if (!isObject(raw) || !isInviteCode(raw.code) || !isObject(raw.community)) return null;
    const { community } = raw;
    if (!isString(community.id) || !isString(community.name)) return null;
    if (!isWholeNumber(raw.member_count) || !isBoolean(raw.usable)) return null;
    return {
        code: raw.code,
        community: { id: community.id, name: community.name },
        memberCount: raw.member_count,
        usable: raw.usable,
    };
};

/** `POST /api/invites/{code}`: the community just joined. */
export const parseJoinResult = (raw: unknown): CommunityId | null =>
    isObject(raw) && isString(raw.community_id) ? raw.community_id : null;
