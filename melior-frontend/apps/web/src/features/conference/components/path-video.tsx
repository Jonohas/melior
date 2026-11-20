import { useEffect, useRef } from 'react';
import { setSpeaker } from '@/features/conference/api';
import { cn } from '@/lib/utils';

type PathVideoProps = {
    stream: MediaStream;
    /** The shown path's element is visible and unmuted; the other one is transparent and muted. */
    shown: boolean;
    /** The speaker to play on; empty or missing means the browser's default. */
    speakerId?: string;
};

/**
 * One path's video of a remote participant. A hidden element keeps decoding and rendering (opacity 0, never
 * `display: none`), so switching to it shows a frame at once.
 */
export const PathVideo = ({ stream, shown, speakerId }: PathVideoProps) => {
    const ref = useRef<HTMLVideoElement>(null);

    useEffect(() => {
        const video = ref.current;
        if (video && speakerId !== undefined) setSpeaker(video, speakerId).catch((e) => console.warn('speaker', e));
    }, [speakerId]);

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
            className={cn(
                'absolute inset-0 size-full bg-black object-contain',
                !shown && 'pointer-events-none opacity-0',
            )}
        />
    );
};
