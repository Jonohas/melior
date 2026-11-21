import { MicOff } from 'lucide-react';
import { useRef } from 'react';
import { Avatar, AvatarFallback, Badge, Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui';
import type { Path, VoiceState } from '@/features/conference/domain';
import { useFullscreen } from '@/features/conference/hooks';
import { cn, initialsOf } from '@/lib/utils';
import { FULLSCREEN_TILE, FullscreenButton } from './fullscreen-button';
import { tilePathLabel } from './media-path-labels';
import { PathVideo } from './path-video';

type RemoteVideoProps = {
    /** The participant's media through the SFU. */
    forwarded?: MediaStream;
    /** The participant's media over the direct link. */
    direct?: MediaStream;
    shownPath: Path;
    username?: string;
    /** Mute and speaking, as the server relays them. */
    voice?: VoiceState;
};

/**
 * One remote participant: their video, or their initials until it arrives. Both paths stay attached and playing;
 * only the shown one is visible. Their sound plays from `RemoteAudio`. A double click or the corner button shows
 * the tile fullscreen.
 */
export const RemoteVideo = ({ forwarded, direct, shownPath, username, voice }: RemoteVideoProps) => {
    const ref = useRef<HTMLDivElement>(null);
    const fullscreen = useFullscreen(ref);
    const hasMedia = forwarded !== undefined || direct !== undefined;
    const path = tilePathLabel(shownPath, username);
    return (
        <div
            ref={ref}
            data-speaking={voice?.speaking === true}
            onDoubleClick={fullscreen.toggle}
            className={cn(
                'relative aspect-video overflow-hidden rounded-lg border-2 border-border bg-muted transition-colors',
                FULLSCREEN_TILE,
                voice?.speaking && 'border-success',
            )}
        >
            {forwarded && <PathVideo stream={forwarded} shown={shownPath === 'forwarded'} />}
            {direct && <PathVideo stream={direct} shown={shownPath === 'direct'} />}
            {!forwarded && !direct && (
                <div className="grid size-full place-items-center">
                    <Avatar className="size-16">
                        <AvatarFallback className="bg-primary/15 text-xl text-primary">
                            {initialsOf(username ?? '?')}
                        </AvatarFallback>
                    </Avatar>
                </div>
            )}
            {voice?.muted && (
                <Badge variant="destructive" className="absolute top-2 left-2">
                    <MicOff /> Muted
                </Badge>
            )}
            <div className="absolute bottom-2 left-2 flex items-center gap-1">
                {username && <Badge variant="secondary">{username}</Badge>}
                {hasMedia && (
                    <Tooltip>
                        <TooltipTrigger asChild>
                            <Badge variant="outline" className="bg-background/70">
                                {path.text}
                            </Badge>
                        </TooltipTrigger>
                        <TooltipContent>{path.hint}</TooltipContent>
                    </Tooltip>
                )}
            </div>
            {hasMedia && <FullscreenButton fullscreen={fullscreen} what={`${username ?? 'this'}'s camera`} />}
        </div>
    );
};
