import { useEffect, useRef } from 'react';

type PathVideoProps = {
    stream: MediaStream;
    /** The shown path's element is visible and unmuted; the other one is transparent and muted. */
    shown: boolean;
};

/**
 * One path's video of a remote participant. A hidden element keeps decoding and rendering (opacity 0, never
 * `display: none`), so switching to it shows a frame at once.
 */
export const PathVideo = ({ stream, shown }: PathVideoProps) => {
    const ref = useRef<HTMLVideoElement>(null);

    useEffect(() => {
        const video = ref.current;
        if (!video) return;
        video.srcObject = stream;
        // A reused media section hands this participant a track that lived in another stream before. The
        // element is paused after its previous tracks ended, so nudge it whenever the track set changes.
        const nudge = () => void video.play().catch(() => undefined);
        stream.addEventListener('addtrack', nudge);
        stream.addEventListener('removetrack', nudge);
        nudge();
        return () => {
            stream.removeEventListener('addtrack', nudge);
            stream.removeEventListener('removetrack', nudge);
        };
    }, [stream]);

    return (
        <video
            ref={ref}
            autoPlay
            playsInline
            muted={!shown}
            data-shown={shown}
            className={`absolute inset-0 h-full w-full bg-black ${shown ? '' : 'opacity-0 pointer-events-none'}`}
        />
    );
};
