import { useEffect, useRef } from 'react';
import { playStream, watchVideoSize } from '@/features/conference/api';
import { aspectOf } from '@/features/conference/domain';
import { cn } from '@/lib/utils';

type PathVideoProps = {
    stream: MediaStream;
    /** The shown path's element is visible; the other one is transparent. */
    shown: boolean;
    /** Hears the frame's width-to-height ratio while this path is shown, so the tile can take its shape. */
    onAspect?: (aspect: number) => void;
};

/**
 * One path's video of a remote participant, always muted: the call's sound plays from `RemoteAudio`, which stays
 * mounted while the grid is not on screen. A hidden element keeps decoding and rendering (opacity 0, never
 * `display: none`), so switching to it shows a frame at once.
 */
export const PathVideo = ({ stream, shown, onAspect }: PathVideoProps) => {
    const ref = useRef<HTMLVideoElement>(null);

    useEffect(() => {
        const video = ref.current;
        if (!video) return;
        return playStream(video, stream);
    }, [stream]);

    useEffect(() => {
        const video = ref.current;
        if (!video || !shown || !onAspect) return;
        return watchVideoSize(video, (width, height) => {
            const aspect = aspectOf(width, height);
            if (aspect !== null) onAspect(aspect);
        });
    }, [shown, onAspect]);

    return (
        <video
            ref={ref}
            autoPlay
            playsInline
            muted
            data-shown={shown}
            className={cn(
                'absolute inset-0 size-full bg-black object-contain',
                !shown && 'pointer-events-none opacity-0',
            )}
        />
    );
};
