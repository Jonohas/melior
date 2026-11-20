import type { UseMutationResult } from '@tanstack/react-query';
import {
    createCommunity,
    createInvite,
    createRole,
    deleteCommunity,
    deleteInvite,
    deleteRole,
    joinInvite,
    removeMember,
    renameCommunity,
    setMemberRoles,
    updateRole,
} from '@/features/communities/api';
import type {
    CommunityId,
    CommunityView,
    Invite,
    NewInvite,
    PositionUpdate,
    RoleChange,
    RoleId,
    UserId,
} from '@/features/communities/domain';
import { queryKeys } from '@/lib/query-keys';
import { useRefreshingMutation } from './refreshing-mutation';

type Mutation<TVariables, TResult = void> = UseMutationResult<TResult, Error, TVariables>;

const detailAndList = (id: CommunityId) => [queryKeys.communities.detail(id), queryKeys.communities.list];

export const useCreateCommunity = (): Mutation<string, CommunityView> =>
    useRefreshingMutation(createCommunity, [queryKeys.communities.list]);

export const useRenameCommunity = (id: CommunityId): Mutation<string> =>
    useRefreshingMutation((name: string) => renameCommunity(id, name), detailAndList(id));

export const useDeleteCommunity = (id: CommunityId): Mutation<void> =>
    useRefreshingMutation(() => deleteCommunity(id), [queryKeys.communities.list]);

/** Leaving is removing yourself from the members. */
export const useLeaveCommunity = (id: CommunityId, myUserId: UserId): Mutation<void> =>
    useRefreshingMutation(() => removeMember(id, myUserId), [queryKeys.communities.list]);

export const useKickMember = (id: CommunityId): Mutation<UserId> =>
    useRefreshingMutation((userId: UserId) => removeMember(id, userId), [queryKeys.communities.members(id)]);

export const useSetMemberRoles = (id: CommunityId): Mutation<{ userId: UserId; roleIds: RoleId[] }> =>
    useRefreshingMutation(
        ({ userId, roleIds }: { userId: UserId; roleIds: RoleId[] }) => setMemberRoles(id, userId, roleIds),
        [queryKeys.communities.members(id), queryKeys.communities.detail(id)],
    );

export const useCreateRole = (id: CommunityId): Mutation<RoleChange> =>
    useRefreshingMutation((role: RoleChange) => createRole(id, role), [queryKeys.communities.detail(id)]);

export const useUpdateRole = (id: CommunityId): Mutation<{ roleId: RoleId; change: RoleChange }> =>
    useRefreshingMutation(
        ({ roleId, change }: { roleId: RoleId; change: RoleChange }) => updateRole(id, roleId, change),
        [queryKeys.communities.detail(id)],
    );

/** Applies a move's position updates one after the other. */
export const useMoveRole = (id: CommunityId): Mutation<PositionUpdate[]> =>
    useRefreshingMutation(
        async (updates: PositionUpdate[]) => {
            for (const { roleId, position } of updates) await updateRole(id, roleId, { position });
        },
        [queryKeys.communities.detail(id)],
    );

export const useDeleteRole = (id: CommunityId): Mutation<RoleId> =>
    useRefreshingMutation(
        (roleId: RoleId) => deleteRole(id, roleId),
        [queryKeys.communities.detail(id), queryKeys.communities.members(id)],
    );

export const useCreateInvite = (id: CommunityId): Mutation<NewInvite, Invite> =>
    useRefreshingMutation((invite: NewInvite) => createInvite(id, invite), [queryKeys.communities.invites(id)]);

export const useDeleteInvite = (id: CommunityId): Mutation<string> =>
    useRefreshingMutation(deleteInvite, [queryKeys.communities.invites(id)]);

/** Joins through an invite; resolves to the community id. */
export const useJoinInvite = (): Mutation<string, CommunityId> =>
    useRefreshingMutation(joinInvite, [queryKeys.communities.list]);
