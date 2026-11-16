import type { Room } from '@/features/rooms/domain';

type RoomAccessSummaryProps = {
    room: Room;
};

/** What a non-owner sees: who owns the room and whether it is open. */
export const RoomAccessSummary = ({ room }: RoomAccessSummaryProps) => {
    return (
        <p>
            Owner: {room.owner} - {room.restricted ? 'Invite only' : 'Open to all members'}
        </p>
    );
};
