import { queryOptions, type UseQueryResult, useQuery } from '@tanstack/react-query';
import { getCommunity, listCommunities, listInvites, listMembers, previewInvite } from '@/features/communities/api';
import type { CommunityId, CommunitySummary, Invite, InvitePreview, Member } from '@/features/communities/domain';
import { queryKeys } from '@/lib/query-keys';

/** One community's view; shared by the route loaders and the components. */
export const communityQueryOptions = (communityId: CommunityId) =>
    queryOptions({
        queryKey: queryKeys.communities.detail(communityId),
        queryFn: () => getCommunity(communityId),
        retry: false,
    });

/** The servers the user is a member of. */
export const useCommunitiesQuery = (): UseQueryResult<CommunitySummary[]> =>
    useQuery({ queryKey: queryKeys.communities.list, queryFn: listCommunities, retry: false });

export const useCommunityQuery = (communityId: CommunityId) => useQuery(communityQueryOptions(communityId));

/** The members, loaded only while something shows them. */
export const useMembersQuery = (communityId: CommunityId, enabled: boolean): UseQueryResult<Member[]> =>
    useQuery({
        queryKey: queryKeys.communities.members(communityId),
        queryFn: () => listMembers(communityId),
        enabled,
    });

export const useInvitesQuery = (communityId: CommunityId, enabled: boolean): UseQueryResult<Invite[]> =>
    useQuery({
        queryKey: queryKeys.communities.invites(communityId),
        queryFn: () => listInvites(communityId),
        enabled,
    });

export const useInvitePreviewQuery = (code: string): UseQueryResult<InvitePreview> =>
    useQuery({ queryKey: queryKeys.invites.preview(code), queryFn: () => previewInvite(code), retry: false });
