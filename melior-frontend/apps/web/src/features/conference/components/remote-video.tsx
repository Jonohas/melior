import { Avatar, AvatarFallback, Badge } from '@/components/ui';
import type { Path } from '@/features/conference/domain';
import { PathVideo } from './path-video';

type RemoteVideoProps = {
    /** The participant's media through the SFU. */
    forwarded?: MediaStream;
    /** The participant's media over the direct link. */
    direct?: MediaStream;
    shownPath: Path;
    username?: string;
    /** The speaker to play this participant's sound on; empty or missing means the browser's default. */
    speakerId?: string;
};

/** One remote participant: their video, or their initials until it arrives. Both paths stay attached and playing; only the shown one is visible and audible. */
export const RemoteVideo = ({ forwarded, direct, shownPath, username, speakerId }: RemoteVideoProps) => (
    <div className="relative aspect-video overflow-hidden rounded-lg border-2 border-success bg-muted">
        {forwarded && <PathVideo stream={forwarded} shown={shownPath === 'forwarded'} speakerId={speakerId} />}
        {direct && <PathVideo stream={direct} shown={shownPath === 'direct'} speakerId={speakerId} />}
        {!forwarded && !direct && (
            <div className="grid size-full place-items-center">
                <Avatar className="size-16">
                    <AvatarFallback className="bg-primary/15 text-xl text-primary">
                        {(username ?? '?').slice(0, 2).toUpperCase()}
                    </AvatarFallback>
                </Avatar>
            </div>
        )}
        {username && (
            <Badge variant="secondary" className="absolute bottom-2 left-2">
                {username}
            </Badge>
        )}
    </div>
);
