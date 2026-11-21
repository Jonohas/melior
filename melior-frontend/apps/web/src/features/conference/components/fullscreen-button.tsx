import { Maximize, Minimize } from 'lucide-react';
import { Button, Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui';
import type { Fullscreen } from '@/features/conference/hooks';

/** Classes of a tile that can go fullscreen: the button shows on hover, and fullscreen drops the frame. */
export const FULLSCREEN_TILE = 'group [&:fullscreen]:rounded-none [&:fullscreen]:border-0';

type FullscreenButtonProps = {
    fullscreen: Fullscreen;
    /** What goes fullscreen, for the label: "andreas's screen". */
    what: string;
};

/**
 * The fullscreen switch in a tile's top-right corner. It shows while the tile is hovered or focused, and stays
 * while fullscreen so the way out is visible. Renders nothing where the browser has no fullscreen.
 */
export const FullscreenButton = ({ fullscreen, what }: FullscreenButtonProps) => {
    if (!fullscreen.supported) return null;
    const label = fullscreen.active ? 'Exit fullscreen' : `Show ${what} fullscreen`;
    return (
        <Tooltip>
            <TooltipTrigger asChild>
                <Button
                    type="button"
                    size="icon-sm"
                    variant="secondary"
                    aria-label={label}
                    aria-pressed={fullscreen.active}
                    data-active={fullscreen.active}
                    onClick={fullscreen.toggle}
                    className="absolute top-2 right-2 opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100 data-[active=true]:opacity-100"
                >
                    {fullscreen.active ? <Minimize /> : <Maximize />}
                </Button>
            </TooltipTrigger>
            <TooltipContent>{label}</TooltipContent>
        </Tooltip>
    );
};
