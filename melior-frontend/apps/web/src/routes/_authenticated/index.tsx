import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { TopBar } from '@/components';
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
        <>
            <TopBar />
            <main className="min-h-0 flex-1 overflow-y-auto">
                <div className="mx-auto flex max-w-5xl flex-col gap-6 p-6">
                    <div>
                        <h1 className="text-2xl font-semibold">Rooms</h1>
                        <p className="text-sm text-muted-foreground">Rooms you own or were invited to.</p>
                    </div>
                    <ChannelForm
                        onSubmit={(channelId) => navigate({ to: '/rooms/$channelId', params: { channelId } })}
                    />
                    <RoomList
                        rooms={rooms.data}
                        loading={rooms.isPending}
                        error={error}
                        onDelete={(channelId) => deleteRoom.mutate(channelId)}
                    />
                </div>
            </main>
        </>
    );
}
