import type { UseMutationResult } from '@tanstack/react-query';
import {
    createCategory,
    createChannel,
    deleteCategory,
    deleteChannel,
    updateCategory,
    updateChannel,
} from '@/features/communities/api';
import type {
    CategoryChange,
    CategoryId,
    Channel,
    ChannelChange,
    CommunityId,
    NewChannel,
} from '@/features/communities/domain';
import { queryKeys } from '@/lib/query-keys';
import { useRefreshingMutation } from './refreshing-mutation';

type Mutation<TVariables, TResult = void> = UseMutationResult<TResult, Error, TVariables>;

const detail = (id: CommunityId) => [queryKeys.communities.detail(id)];

export const useCreateCategory = (id: CommunityId): Mutation<string> =>
    useRefreshingMutation((name: string) => createCategory(id, name), detail(id));

export const useUpdateCategory = (id: CommunityId): Mutation<{ categoryId: CategoryId; change: CategoryChange }> =>
    useRefreshingMutation(
        ({ categoryId, change }: { categoryId: CategoryId; change: CategoryChange }) =>
            updateCategory(id, categoryId, change),
        detail(id),
    );

export const useDeleteCategory = (id: CommunityId): Mutation<CategoryId> =>
    useRefreshingMutation((categoryId: CategoryId) => deleteCategory(id, categoryId), detail(id));

export const useCreateChannel = (id: CommunityId): Mutation<NewChannel, Channel> =>
    useRefreshingMutation((channel: NewChannel) => createChannel(id, channel), detail(id));

export const useUpdateChannel = (id: CommunityId): Mutation<{ channelId: string; change: ChannelChange }> =>
    useRefreshingMutation(
        ({ channelId, change }: { channelId: string; change: ChannelChange }) => updateChannel(channelId, change),
        detail(id),
    );

export const useDeleteChannel = (id: CommunityId): Mutation<string> => useRefreshingMutation(deleteChannel, detail(id));
