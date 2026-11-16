import { type UseMutationResult, useMutation, useQueryClient } from '@tanstack/react-query';
import { deleteRoom } from '@/features/rooms/api';
import { queryKeys } from '@/lib/query-keys';

export const useDeleteRoomMutation = (): UseMutationResult<void, Error, string> => {
    const queryClient = useQueryClient();
    return useMutation({
        mutationFn: deleteRoom,
        onSuccess: () => queryClient.invalidateQueries({ queryKey: queryKeys.rooms.all }),
    });
};
