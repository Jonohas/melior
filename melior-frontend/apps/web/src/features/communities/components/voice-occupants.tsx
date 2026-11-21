import { MicOff, Video } from 'lucide-react';
import { Avatar, AvatarFallback } from '@/components/ui';
import type { VoiceEntry } from '@/features/communities/domain';
import { cn, initialsOf } from '@/lib/utils';

const OccupantRow = ({ entry }: { entry: VoiceEntry }) => (
    <div
        className="flex items-center gap-2 rounded-md px-2 py-0.5 text-sm text-muted-foreground"
        data-speaking={entry.speaking}
    >
        <Avatar
            className={cn(
                'size-6 ring-2 ring-transparent ring-offset-1 ring-offset-card transition-shadow',
                entry.speaking && 'ring-success',
            )}
        >
            <AvatarFallback className="bg-primary/15 text-[10px] text-primary">
                {initialsOf(entry.username)}
            </AvatarFallback>
        </Avatar>
        <span className={cn('flex-1 truncate', entry.speaking && 'text-foreground')}>{entry.username}</span>
        {entry.screen && (
            <span className="rounded-sm bg-destructive px-1 text-[10px] font-bold leading-4 text-white">LIVE</span>
        )}
        {entry.camera && <Video className="size-3.5 shrink-0" aria-label="Camera on" />}
        {entry.muted && <MicOff className="size-3.5 shrink-0 text-destructive" aria-label="Muted" />}
    </div>
);

/** Who is in a voice channel, under its name in the channel list. */
export const VoiceOccupants = ({ occupants }: { occupants: VoiceEntry[] }) => {
    if (occupants.length === 0) return null;
    return (
        <ul className="ml-6 flex list-none flex-col gap-0.5 pb-1" aria-label="In this channel">
            {occupants.map((entry) => (
                <li key={entry.sessionId}>
                    <OccupantRow entry={entry} />
                </li>
            ))}
        </ul>
    );
};
