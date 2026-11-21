import { type CommunityId, type Member, parseMembers, type RoleId, type UserId } from '@/features/communities/domain';
import { fetchParsed, segment, send, withId, withJson } from './http';

const memberUrl = (communityId: CommunityId, userId?: UserId) =>
    withId(`/api/communities/${segment(communityId)}/members`, userId);

export const listMembers = (communityId: CommunityId): Promise<Member[]> =>
    fetchParsed(memberUrl(communityId), parseMembers, 'member list');

/** Kicks a member, or leaves the community when `userId` is the caller's own. */
export const removeMember = async (communityId: CommunityId, userId: UserId): Promise<void> => {
    await send(memberUrl(communityId, userId), { method: 'DELETE' });
};

/** Replaces a member's roles. */
export const setMemberRoles = async (communityId: CommunityId, userId: UserId, roleIds: RoleId[]): Promise<void> => {
    await send(`${memberUrl(communityId, userId)}/roles`, withJson('PUT', { role_ids: roleIds }));
};
