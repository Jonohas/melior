import { Link } from '@tanstack/react-router';
import type { Room } from '@/features/rooms/domain';

type RoomListProps = {
    rooms: Room[] | undefined;
    loading: boolean;
    error: string | null;
    onDelete: (channelId: string) => void;
};

export const RoomList = ({ rooms, loading, error, onDelete }: RoomListProps) => {
    return (
        <section className="flex flex-col items-center gap-2" aria-label="Your rooms">
            <h2 className="text-heading4">Your rooms</h2>
            {loading && <p>Loading…</p>}
            {error && (
                <p className="text-red-500" role="alert">
                    {error}
                </p>
            )}
            {rooms && rooms.length === 0 && <p>No rooms yet. Join one below to create it.</p>}
            {rooms && rooms.length > 0 && (
                <ul className="flex flex-col gap-1">
                    {rooms.map((room) => (
                        <li key={room.channelId} className="flex items-center gap-3">
                            <span className="font-semibold">{room.channelId}</span>
                            <span>owner: {room.owner}</span>
                            <span>{room.restricted ? 'invite only' : 'open'}</span>
                            <Link to="/rooms/$channelId" params={{ channelId: room.channelId }} className="underline">
                                Join
                            </Link>
                            {room.isOwner && (
                                <button type="button" className="underline" onClick={() => onDelete(room.channelId)}>
                                    Delete
                                </button>
                            )}
                        </li>
                    ))}
                </ul>
            )}
        </section>
    );
};
