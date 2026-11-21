import { type CommunityId, type RoleChange, type RoleId, toRoleBody } from '@/features/communities/domain';
import { segment, send, withId, withJson } from './http';

const roleUrl = (communityId: CommunityId, roleId?: RoleId) =>
    withId(`/api/communities/${segment(communityId)}/roles`, roleId);

/** Creates a role just below the caller's rank. */
export const createRole = async (communityId: CommunityId, role: RoleChange): Promise<void> => {
    await send(roleUrl(communityId), withJson('POST', toRoleBody(role)));
};

export const updateRole = async (communityId: CommunityId, roleId: RoleId, change: RoleChange): Promise<void> => {
    await send(roleUrl(communityId, roleId), withJson('PATCH', toRoleBody(change)));
};

export const deleteRole = async (communityId: CommunityId, roleId: RoleId): Promise<void> => {
    await send(roleUrl(communityId, roleId), { method: 'DELETE' });
};
