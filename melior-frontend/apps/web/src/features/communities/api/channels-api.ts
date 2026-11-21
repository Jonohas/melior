import {
    type CategoryChange,
    type CategoryId,
    type Channel,
    type ChannelChange,
    type CommunityId,
    CommunityRequestError,
    type NewChannel,
    parseChannel,
    toCategoryPatchBody,
    toChannelPatchBody,
    toNewChannelBody,
} from '@/features/communities/domain';
import type { ChannelProbe } from '@/features/conference/domain';
import { fetchParsed, segment, send, withId, withJson } from './http';

const categoryUrl = (communityId: CommunityId, categoryId?: CategoryId) =>
    withId(`/api/communities/${segment(communityId)}/categories`, categoryId);
const channelUrl = (channelId: string) => withId('/api/channels', channelId);

export const createCategory = async (communityId: CommunityId, name: string): Promise<void> => {
    await send(categoryUrl(communityId), withJson('POST', { name }));
};

export const updateCategory = async (
    communityId: CommunityId,
    categoryId: CategoryId,
    change: CategoryChange,
): Promise<void> => {
    await send(categoryUrl(communityId, categoryId), withJson('PATCH', toCategoryPatchBody(change)));
};

/** Its channels move out of any category. */
export const deleteCategory = async (communityId: CommunityId, categoryId: CategoryId): Promise<void> => {
    await send(categoryUrl(communityId, categoryId), { method: 'DELETE' });
};

export const createChannel = (communityId: CommunityId, channel: NewChannel): Promise<Channel> =>
    fetchParsed(
        `/api/communities/${segment(communityId)}/channels`,
        parseChannel,
        'channel',
        withJson('POST', toNewChannelBody(channel)),
    );

export const updateChannel = async (channelId: string, change: ChannelChange): Promise<void> => {
    await send(channelUrl(channelId), withJson('PATCH', toChannelPatchBody(change)));
};

export const deleteChannel = async (channelId: string): Promise<void> => {
    await send(channelUrl(channelId), { method: 'DELETE' });
};

const PROBE_BY_STATUS: Partial<Record<number, ChannelProbe>> = { 403: 'forbidden', 404: 'not_found', 409: 'full' };

/**
 * May the user join the voice channel now, for the call's reconnect logic (the browser shows no status for a
 * refused WebSocket upgrade): `GET /api/channels/{id}/admission` answers 204, 404, 403 or 409.
 */
export const probeVoiceChannel = async (channelId: string): Promise<ChannelProbe> => {
    try {
        await send(`${channelUrl(channelId)}/admission`);
        return 'ok';
    } catch (error) {
        if (error instanceof CommunityRequestError) return PROBE_BY_STATUS[error.status] ?? 'unknown';
        return 'unknown';
    }
};
