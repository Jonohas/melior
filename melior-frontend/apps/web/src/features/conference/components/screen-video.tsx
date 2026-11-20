import { MonitorUp } from 'lucide-react';
import { Badge } from '@/components/ui';
import type { Path } from '@/features/conference/domain';
import { PathVideo } from './path-video';

type ScreenVideoProps = {
    /** The screen through the SFU. */
    forwarded?: MediaStream;
    /** The screen over the direct link, while the sharer says it shares one. */
    direct?: MediaStream;
    /** The path the sharer's media is shown over: its camera and its screen always share one path. */
    shownPath: Path;
    /** Who is sharing. */
    username?: string;
};

/**
 * A screen someone shares: shown large and uncropped, so text on it stays readable. Like the camera tile it
 * keeps one element per path attached, and only the shown path's is visible.
 */
export const ScreenVideo = ({ forwarded, direct, shownPath, username }: ScreenVideoProps) => (
    <section
        aria-label={`${username ?? 'Someone'}'s screen`}
        className="relative aspect-video w-full overflow-hidden rounded-lg border border-border bg-black"
    >
        {forwarded && <PathVideo stream={forwarded} shown={shownPath === 'forwarded'} />}
        {direct && <PathVideo stream={direct} shown={shownPath === 'direct'} />}
        <Badge variant="secondary" className="absolute bottom-2 left-2">
            <MonitorUp /> {username ? `${username} is sharing their screen` : 'Shared screen'}
        </Badge>
    </section>
);
