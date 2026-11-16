import { type UseQueryResult, useQuery } from '@tanstack/react-query';
import { getRoom } from '@/features/rooms/api';
import { type Room, RoomNotFoundError } from '@/features/rooms/domain';
import { queryKeys } from '@/lib/query-keys';

/** One room, or null while nobody has registered it (the first join registers it). */
export const useRoomQuery = (channelId: string): UseQueryResult<Room | null> =>
    useQuery({
        queryKey: queryKeys.rooms.detail(channelId),
        queryFn: async () => {
            try {
                return await getRoom(channelId);
            } catch (error) {
                if (error instanceof RoomNotFoundError) return null;
                throw error;
            }
        },
        retry: false,
    });
