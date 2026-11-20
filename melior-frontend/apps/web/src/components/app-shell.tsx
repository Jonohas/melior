import { Link } from '@tanstack/react-router';
import { LogOut } from 'lucide-react';
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
import { initialsOf } from '@/lib/utils';
import { MeliorLogo } from './melior-logo';

type AppShellProps = {
    username: string;
    /** The server rail: one entry per server the user is in. */
    rail: ReactNode;
    /** Shown over the bottom left of the page, beside the rail: the call panel on pages without a channel column. */
    corner?: ReactNode;
    children: ReactNode;
};

const UserMenu = ({ username }: { username: string }) => (
    <DropdownMenu>
        <DropdownMenuTrigger asChild>
            <Button variant="ghost" className="size-12 rounded-full p-0" aria-label={`Account: ${username}`}>
                <Avatar className="size-10">
                    <AvatarFallback className="bg-primary/15 text-xs font-medium text-primary">
                        {initialsOf(username)}
                    </AvatarFallback>
                </Avatar>
            </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent side="right" align="end" className="w-56">
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

/** The frame of every signed-in page: the server rail with the user menu, and the page beside it. */
export const AppShell = ({ username, rail, corner, children }: AppShellProps) => (
    <div className="flex h-screen overflow-hidden">
        <aside
            aria-label="Servers"
            className="flex w-[72px] shrink-0 flex-col items-center gap-2 border-r border-border bg-background py-3"
        >
            <Link to="/" aria-label="Home" className="grid size-12 place-items-center rounded-2xl hover:bg-accent">
                <MeliorLogo className="size-8" />
            </Link>
            <div className="h-px w-8 bg-border" />
            <nav className="flex min-h-0 w-full flex-1 flex-col items-center gap-2 overflow-y-auto">{rail}</nav>
            <UserMenu username={username} />
        </aside>
        <div className="relative flex min-w-0 flex-1">
            {children}
            {corner && (
                <div className="absolute bottom-3 left-3 z-40 w-60 overflow-hidden rounded-lg shadow-lg">{corner}</div>
            )}
        </div>
    </div>
);
