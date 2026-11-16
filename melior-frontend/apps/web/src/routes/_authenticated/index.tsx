import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { ChannelForm } from '@/features/conference/components';
import { RoomList } from '@/features/rooms/components';
import { useDeleteRoomMutation, useRoomsQuery } from '@/features/rooms/hooks';

export const Route = createFileRoute('/_authenticated/')({
    component: LobbyPage,
});

function LobbyPage() {
    const navigate = useNavigate();
    const rooms = useRoomsQuery();
    const deleteRoom = useDeleteRoomMutation();
    const error = rooms.error?.message ?? deleteRoom.error?.message ?? null;

    return (
        <div className="flex items-center flex-col w-full min-h-screen gap-6 p-4">
            <h1 className="text-heading3">Melior</h1>
            <RoomList
                rooms={rooms.data}
                loading={rooms.isPending}
                error={error}
                onDelete={(channelId) => deleteRoom.mutate(channelId)}
            />
            <ChannelForm onSubmit={(channelId) => navigate({ to: '/rooms/$channelId', params: { channelId } })} />
        </div>
    );
}
