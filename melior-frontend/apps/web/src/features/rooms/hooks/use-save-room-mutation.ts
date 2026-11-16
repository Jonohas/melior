import { type UseMutationResult, useMutation, useQueryClient } from '@tanstack/react-query';
import { saveRoom } from '@/features/rooms/api';
import type { Room, RoomSettings } from '@/features/rooms/domain';
import { queryKeys } from '@/lib/query-keys';

/** Saves the room's access. The server's answer replaces the cached room; the list is refetched. */
export const useSaveRoomMutation = (channelId: string): UseMutationResult<Room, Error, RoomSettings> => {
    const queryClient = useQueryClient();
    return useMutation({
        mutationFn: (settings: RoomSettings) => saveRoom(channelId, settings),
        onSuccess: (room) => {
            queryClient.setQueryData(queryKeys.rooms.detail(channelId), room);
            void queryClient.invalidateQueries({ queryKey: queryKeys.rooms.list });
        },
    });
};
