import { useEffect, useRef } from 'react';
import { playStream } from '@/features/conference/api';
import { cn } from '@/lib/utils';

type PathVideoProps = {
    stream: MediaStream;
    /** The shown path's element is visible; the other one is transparent. */
    shown: boolean;
};

/**
 * One path's video of a remote participant, always muted: the call's sound plays from `RemoteAudio`, which stays
 * mounted while the grid is not on screen. A hidden element keeps decoding and rendering (opacity 0, never
 * `display: none`), so switching to it shows a frame at once.
 */
export const PathVideo = ({ stream, shown }: PathVideoProps) => {
    const ref = useRef<HTMLVideoElement>(null);

    useEffect(() => {
        const video = ref.current;
        if (!video) return;
        return playStream(video, stream);
    }, [stream]);

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
