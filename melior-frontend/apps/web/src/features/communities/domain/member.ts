import type { RoleId, UserId } from './community';
import { isObject, isString, parseList } from './wire';

/** A member of a community, as `GET /api/communities/{id}/members` lists them. */
export type Member = {
    userId: UserId;
    username: string;
    /** Never contains `@everyone`: every member has it implicitly. */
    roleIds: RoleId[];
};

export const parseMember = (raw: unknown): Member | null => {
    if (!isObject(raw) || !isString(raw.user_id) || !isString(raw.username)) return null;
    if (!Array.isArray(raw.role_ids) || !raw.role_ids.every(isString)) return null;
    return { userId: raw.user_id, username: raw.username, roleIds: raw.role_ids };
};

/** Validates a member list; sorted by username for display. */
export const parseMembers = (raw: unknown): Member[] | null =>
    parseList(raw, parseMember)?.sort((a, b) => a.username.localeCompare(b.username)) ?? null;
