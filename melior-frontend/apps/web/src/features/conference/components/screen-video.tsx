import { MonitorUp } from 'lucide-react';
import { useRef, useState } from 'react';
import { Badge } from '@/components/ui';
import { DEFAULT_SCREEN_ASPECT, type Path } from '@/features/conference/domain';
import { useFullscreen } from '@/features/conference/hooks';
import { cn } from '@/lib/utils';
import { FULLSCREEN_TILE, FullscreenButton } from './fullscreen-button';
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
 * The largest box of the screen's own shape that fits its cell: the cell is a size container, and the box takes
 * the smaller of the cell's width and the cell's height times the ratio. An ultrawide or portrait screen then
 * fills the cell instead of sitting in a 16:9 frame with bars.
 */
const fittedBox = (aspect: number) => ({
    aspectRatio: aspect,
    width: `min(100cqw, calc(100cqh * ${aspect}))`,
});

/**
 * A screen someone shares: shown large and uncropped, so text on it stays readable. Like the camera tile it
 * keeps one element per path attached, and only the shown path's is visible. A double click or the corner
 * button shows it fullscreen.
 */
export const ScreenVideo = ({ forwarded, direct, shownPath, username }: ScreenVideoProps) => {
    const ref = useRef<HTMLElement>(null);
    const fullscreen = useFullscreen(ref);
    const [aspect, setAspect] = useState(DEFAULT_SCREEN_ASPECT);
    const label = `${username ?? 'Someone'}'s screen`;
    return (
        <section
            ref={ref}
            aria-label={label}
            onDoubleClick={fullscreen.toggle}
            style={fittedBox(aspect)}
            className={cn('relative overflow-hidden rounded-lg border border-border bg-black', FULLSCREEN_TILE)}
        >
            {forwarded && <PathVideo stream={forwarded} shown={shownPath === 'forwarded'} onAspect={setAspect} />}
            {direct && <PathVideo stream={direct} shown={shownPath === 'direct'} onAspect={setAspect} />}
            <Badge variant="secondary" className="absolute bottom-2 left-2">
                <MonitorUp /> {username ? `${username} is sharing their screen` : 'Shared screen'}
            </Badge>
            <FullscreenButton fullscreen={fullscreen} what={label} />
        </section>
    );
};
