import type { Role, UserId } from './community';
import type { Member } from './member';
import { rolesByRank } from './roles';
import type { VoiceRoster } from './voice-roster';

/** One section of the member list: the members whose highest role is `role`, or those without one (`null`). */
export type MemberGroup = { role: Role | null; members: Member[] };

/** The member's highest role, ties going to the one listed first; null without any. */
export const highestRole = (member: Pick<Member, 'roleIds'>, roles: readonly Role[]): Role | null =>
    rolesByRank(roles).find((role) => member.roleIds.includes(role.id)) ?? null;

/** The colour of a member's name: their highest role that has one, as in Discord; null for the default. */
export const nameColor = (member: Pick<Member, 'roleIds'>, roles: readonly Role[]): number | null =>
    rolesByRank(roles).find((role) => role.color !== null && member.roleIds.includes(role.id))?.color ?? null;

const byUsername = (a: Member, b: Member): number =>
    a.username.localeCompare(b.username) || a.userId.localeCompare(b.userId);

/**
 * The member list, Discord style: one group per role that is someone's highest, highest role first, then the
 * members without a role. Each group is sorted by username; empty groups are left out.
 */
export const groupMembers = (members: readonly Member[], roles: readonly Role[]): MemberGroup[] => {
    const byRole = new Map<string | null, Member[]>();
    for (const member of members) {
        const key = highestRole(member, roles)?.id ?? null;
        byRole.set(key, [...(byRole.get(key) ?? []), member]);
    }
    const groups: MemberGroup[] = rolesByRank(roles).map((role) => ({ role, members: byRole.get(role.id) ?? [] }));
    groups.push({ role: null, members: byRole.get(null) ?? [] });
    return groups
        .filter((group) => group.members.length > 0)
        .map((group) => ({ ...group, members: [...group.members].sort(byUsername) }));
};

/** The users in any voice channel of the community right now. */
export const usersInVoice = (roster: VoiceRoster): ReadonlySet<UserId> =>
    new Set(Object.values(roster).flatMap((entries) => Object.values(entries).map((entry) => entry.userId)));

/** Reads the stored open or closed state of the member list; null when nothing (valid) was stored. */
export const parseMemberListOpen = (raw: string | null): boolean | null => {
    if (raw === 'true') return true;
    if (raw === 'false') return false;
    return null;
};

/** Whether the member list starts open: as the person left it, else only on a screen wide enough for it. */
export const initialMemberListOpen = (stored: boolean | null, wideScreen: boolean): boolean => stored ?? wideScreen;
