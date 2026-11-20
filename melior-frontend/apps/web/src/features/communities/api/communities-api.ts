import {
    type CommunityId,
    type CommunitySummary,
    type CommunityView,
    parseCommunitySummaries,
    parseCommunityView,
} from '@/features/communities/domain';
import { fetchParsed, send, withId, withJson } from './http';

const communityUrl = (id: CommunityId) => withId('/api/communities', id);

/** The communities the user is a member of. */
export const listCommunities = (): Promise<CommunitySummary[]> =>
    fetchParsed('/api/communities', parseCommunitySummaries, 'community list');

/** Creates a community owned by the caller, seeded with a text and a voice channel. */
export const createCommunity = (name: string): Promise<CommunityView> =>
    fetchParsed('/api/communities', parseCommunityView, 'community', withJson('POST', { name }));

/** Everything the caller may see of one community. 404 unknown, 403 not a member. */
export const getCommunity = (id: CommunityId): Promise<CommunityView> =>
    fetchParsed(communityUrl(id), parseCommunityView, 'community');

/** Owner or administrator. */
export const renameCommunity = async (id: CommunityId, name: string): Promise<void> => {
    await send(communityUrl(id), withJson('PATCH', { name }));
};

/** Owner only: deletes the community with its channels and their history. */
export const deleteCommunity = async (id: CommunityId): Promise<void> => {
    await send(communityUrl(id), { method: 'DELETE' });
};
