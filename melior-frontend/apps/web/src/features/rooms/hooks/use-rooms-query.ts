import { type UseQueryResult, useQuery } from '@tanstack/react-query';
import { listRooms } from '@/features/rooms/api';
import type { Room } from '@/features/rooms/domain';
import { queryKeys } from '@/lib/query-keys';

/** "Your rooms": rooms the user owns or is invited to. */
export const useRoomsQuery = (): UseQueryResult<Room[]> =>
    useQuery({ queryKey: queryKeys.rooms.list, queryFn: listRooms, retry: false });
