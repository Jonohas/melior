import type { CommunityView, Me, Role } from './community';
import type { Member } from './member';
import { hasPermission, PERMISSIONS, type Permission } from './permissions';

/** What the caller may do in the community, to show only the controls that work. The server checks again. */
export type Abilities = {
    manageChannels: boolean;
    manageRoles: boolean;
    kickMembers: boolean;
    createInvite: boolean;
    /** The invite list: `manage_channels` or `administrator`. */
    viewInvites: boolean;
    renameServer: boolean;
    deleteServer: boolean;
    leaveServer: boolean;
    /** Any tab of the server settings is useful. */
    openServerSettings: boolean;
};

export const abilitiesOf = (me: Me): Abilities => {
    const can = (permission: Permission) => me.isOwner || hasPermission(me.permissions, permission);
    const manageChannels = can('manage_channels');
    const manageRoles = can('manage_roles');
    const kickMembers = can('kick_members');
    const renameServer = can('administrator');
    return {
        manageChannels,
        manageRoles,
        kickMembers,
        createInvite: can('create_invite'),
        viewInvites: manageChannels,
        renameServer,
        deleteServer: me.isOwner,
        leaveServer: !me.isOwner,
        openServerSettings: renameServer || manageRoles || kickMembers || manageChannels,
    };
};

/** `@everyone`'s id is the community's own id. */
export const isEveryoneRole = (view: Pick<CommunityView, 'id'>, role: Pick<Role, 'id'>): boolean => role.id === view.id;

/** A member's rank: the highest position among their roles, 0 without any. */
export const memberRank = (member: Pick<Member, 'roleIds'>, roles: readonly Role[]): number =>
    roles.filter((role) => member.roleIds.includes(role.id)).reduce((rank, role) => Math.max(rank, role.position), 0);

const outranks = (me: Me, position: number): boolean => me.isOwner || position < me.rank;

/** May the caller edit (or delete) this role: `manage_roles` and the role below their rank. */
export const canEditRole = (me: Me, role: Role): boolean => abilitiesOf(me).manageRoles && outranks(me, role.position);

/** May the caller give or take this role: `manage_roles`, below their rank, and not `@everyone`. */
export const canAssignRole = (view: CommunityView, role: Role): boolean =>
    !isEveryoneRole(view, role) && canEditRole(view.me, role);

/** The permissions the caller may grant to a role: their own, or all for the owner and administrators. */
export const grantablePermissions = (me: Me): readonly Permission[] =>
    me.isOwner || me.permissions.includes('administrator') ? PERMISSIONS : me.permissions;

/** May the caller kick this member: `kick_members`, not the owner, not themselves, and below their rank. */
export const canKick = (view: CommunityView, member: Member): boolean => {
    const { me } = view;
    if (!abilitiesOf(me).kickMembers) return false;
    if (member.userId === view.ownerId || member.userId === me.userId) return false;
    return outranks(me, memberRank(member, view.roles));
};
