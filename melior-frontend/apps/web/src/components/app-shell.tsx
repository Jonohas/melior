import { Link } from '@tanstack/react-router';
import { ChevronsUpDown, Hash, Lock, LogOut, Video } from 'lucide-react';
import type { ReactNode } from 'react';
import {
    Avatar,
    AvatarFallback,
    Button,
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from '@/components/ui';
import { MeliorLogo } from './melior-logo';

export type ShellRoom = {
    channelId: string;
    restricted: boolean;
};

type AppShellProps = {
    username: string;
    /** The rooms listed in the sidebar. */
    rooms: ShellRoom[];
    children: ReactNode;
};

const NAV_LINK =
    'flex items-center gap-2 rounded-md px-2 py-1.5 text-sm text-muted-foreground hover:bg-accent hover:text-accent-foreground';
const NAV_LINK_ACTIVE = 'bg-accent font-medium text-accent-foreground';

const initialsOf = (name: string): string => name.slice(0, 2).toUpperCase();

const RoomLinks = ({ rooms }: { rooms: ShellRoom[] }) => (
    <ul className="flex flex-col gap-0.5">
        {rooms.map((room) => (
            <li key={room.channelId}>
                <Link
                    to="/rooms/$channelId"
                    params={{ channelId: room.channelId }}
                    className={NAV_LINK}
                    activeProps={{ className: NAV_LINK_ACTIVE }}
                >
                    {room.restricted ? (
                        <Lock className="size-4 shrink-0" aria-label="Invite only" />
                    ) : (
                        <Hash className="size-4 shrink-0" aria-hidden="true" />
                    )}
                    <span className="truncate">{room.channelId}</span>
                </Link>
            </li>
        ))}
    </ul>
);

const UserMenu = ({ username }: { username: string }) => (
    <DropdownMenu>
        <DropdownMenuTrigger asChild>
            <Button variant="ghost" className="h-auto w-full justify-start gap-2 px-2 py-2">
                <Avatar className="size-8">
                    <AvatarFallback className="bg-primary/15 text-xs font-medium text-primary">
                        {initialsOf(username)}
                    </AvatarFallback>
                </Avatar>
                <span className="flex-1 truncate text-left">{username}</span>
                <ChevronsUpDown className="size-4 text-muted-foreground" aria-hidden="true" />
            </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent side="top" align="start" className="w-56">
            <DropdownMenuLabel>Signed in as {username}</DropdownMenuLabel>
            <DropdownMenuSeparator />
            {/* A plain anchor: sign-out is a full-page navigation through the BFF, not a client route. */}
            <DropdownMenuItem asChild>
                <a href="/signout">
                    <LogOut /> Sign out
                </a>
            </DropdownMenuItem>
        </DropdownMenuContent>
    </DropdownMenu>
);

/** The frame of every signed-in page: a sidebar with the rooms and the user menu, and the page beside it. */
export const AppShell = ({ username, rooms, children }: AppShellProps) => (
    <div className="flex h-screen overflow-hidden">
        <aside aria-label="Navigation" className="hidden w-64 shrink-0 flex-col border-r border-border bg-card md:flex">
            <div className="flex items-center gap-2 p-4">
                <MeliorLogo className="size-8" />
                <span className="text-lg font-semibold">Melior</span>
            </div>
            <nav className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-2">
                <Link
                    to="/"
                    activeOptions={{ exact: true }}
                    className={NAV_LINK}
                    activeProps={{ className: NAV_LINK_ACTIVE }}
                >
                    <Video className="size-4" aria-hidden="true" /> Rooms
                </Link>
                {rooms.length > 0 && (
                    <div className="flex flex-col gap-1">
                        <h2 className="px-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                            Your rooms
                        </h2>
                        <RoomLinks rooms={rooms} />
                    </div>
                )}
            </nav>
            <div className="border-t border-border p-2">
                <UserMenu username={username} />
            </div>
        </aside>
        <div className="flex min-w-0 flex-1 flex-col">{children}</div>
    </div>
);
