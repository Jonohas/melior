import { ChevronDown, FolderPlus, LogOut, PlusCircle, Settings, UserPlus } from 'lucide-react';
import {
    Button,
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from '@/components/ui';
import type { Abilities } from '@/features/communities/domain';

/** Which of the sidebar's dialogs is open. */
export type SidebarDialog =
    | { kind: 'none' }
    | { kind: 'settings' }
    | { kind: 'invite' }
    | { kind: 'create-channel'; categoryId: string | null }
    | { kind: 'create-category' }
    | { kind: 'channel-settings'; channelId: string }
    | { kind: 'category-settings'; categoryId: string }
    | { kind: 'leave' };

type ServerMenuProps = {
    name: string;
    abilities: Abilities;
    onOpen: (dialog: SidebarDialog) => void;
};

/** The server's name, opening what the caller may do with it. */
export const ServerMenu = ({ name, abilities, onOpen }: ServerMenuProps) => (
    <DropdownMenu>
        <DropdownMenuTrigger asChild>
            <Button variant="ghost" className="h-12 w-full justify-between rounded-none border-b border-border px-4">
                <span className="truncate font-semibold">{name}</span>
                <ChevronDown className="size-4" />
            </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="w-56">
            {abilities.createInvite && (
                <DropdownMenuItem onSelect={() => onOpen({ kind: 'invite' })}>
                    <UserPlus /> Invite people
                </DropdownMenuItem>
            )}
            {abilities.openServerSettings && (
                <DropdownMenuItem onSelect={() => onOpen({ kind: 'settings' })}>
                    <Settings /> Server settings
                </DropdownMenuItem>
            )}
            {abilities.manageChannels && (
                <>
                    <DropdownMenuItem onSelect={() => onOpen({ kind: 'create-channel', categoryId: null })}>
                        <PlusCircle /> Create channel
                    </DropdownMenuItem>
                    <DropdownMenuItem onSelect={() => onOpen({ kind: 'create-category' })}>
                        <FolderPlus /> Create category
                    </DropdownMenuItem>
                </>
            )}
            {abilities.leaveServer && (
                <>
                    <DropdownMenuSeparator />
                    <DropdownMenuItem variant="destructive" onSelect={() => onOpen({ kind: 'leave' })}>
                        <LogOut /> Leave server
                    </DropdownMenuItem>
                </>
            )}
        </DropdownMenuContent>
    </DropdownMenu>
);
