import { Link } from '@tanstack/react-router';
import { Lock, MessageSquare, Trash2, TriangleAlert } from 'lucide-react';
import {
    Alert,
    AlertDescription,
    Badge,
    Button,
    Card,
    CardAction,
    CardContent,
    CardDescription,
    CardFooter,
    CardHeader,
    CardTitle,
    Skeleton,
} from '@/components/ui';
import type { Room } from '@/features/rooms/domain';

type RoomListProps = {
    rooms: Room[] | undefined;
    loading: boolean;
    error: string | null;
    onDelete: (channelId: string) => void;
};

const RoomCard = ({ room, onDelete }: { room: Room; onDelete: (channelId: string) => void }) => (
    <Card>
        <CardHeader>
            <CardTitle className="break-all">{room.channelId}</CardTitle>
            <CardDescription>
                Owner: {room.owner}
                {room.isOwner ? ' (you)' : ''}
            </CardDescription>
            <CardAction>
                <Badge variant="outline">
                    {room.restricted ? (
                        <>
                            <Lock /> Invite only
                        </>
                    ) : (
                        'Open to all members'
                    )}
                </Badge>
            </CardAction>
        </CardHeader>
        <CardFooter className="gap-2">
            <Button asChild>
                <Link to="/rooms/$channelId" params={{ channelId: room.channelId }}>
                    Join
                </Link>
            </Button>
            {room.isOwner && (
                <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="text-destructive"
                    aria-label={`Delete ${room.channelId}`}
                    onClick={() => onDelete(room.channelId)}
                >
                    <Trash2 />
                </Button>
            )}
        </CardFooter>
    </Card>
);

export const RoomList = ({ rooms, loading, error, onDelete }: RoomListProps) => {
    return (
        <section className="flex flex-col gap-3" aria-label="Your rooms">
            <h2 className="text-lg font-semibold">Your rooms</h2>
            {error && (
                <Alert variant="destructive">
                    <TriangleAlert />
                    <AlertDescription>{error}</AlertDescription>
                </Alert>
            )}
            {loading && (
                <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3" aria-busy="true">
                    {[1, 2, 3].map((n) => (
                        <Skeleton key={n} className="h-32" />
                    ))}
                </div>
            )}
            {rooms && rooms.length === 0 && (
                <Card>
                    <CardContent className="flex flex-col items-center gap-2 py-10 text-center">
                        <MessageSquare className="size-6 text-muted-foreground" aria-hidden="true" />
                        <p className="font-medium">No rooms yet</p>
                        <p className="text-sm text-muted-foreground">Join one above to create it.</p>
                    </CardContent>
                </Card>
            )}
            {rooms && rooms.length > 0 && (
                <ul className="grid list-none gap-4 sm:grid-cols-2 xl:grid-cols-3">
                    {rooms.map((room) => (
                        <li key={room.channelId}>
                            <RoomCard room={room} onDelete={onDelete} />
                        </li>
                    ))}
                </ul>
            )}
        </section>
    );
};
