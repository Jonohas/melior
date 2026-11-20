import { MonitorUp } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { Badge } from '@/components/ui';

type ScreenVideoProps = {
    stream: MediaStream;
    /** Who is sharing. */
    username?: string;
};

/** A screen someone shares: shown large and uncropped, so text on it stays readable. */
export const ScreenVideo = ({ stream, username }: ScreenVideoProps) => {
    const ref = useRef<HTMLVideoElement>(null);

    useEffect(() => {
        const video = ref.current;
        if (!video) return;
        video.srcObject = stream;
        // A reused media section hands this stream a track that lived in another stream before, so the
        // element may be paused; nudge it whenever the track set changes.
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
        <section
            aria-label={`${username ?? 'Someone'}'s screen`}
            className="relative aspect-video w-full overflow-hidden rounded-lg border border-border bg-black"
        >
            <video ref={ref} autoPlay playsInline muted className="size-full object-contain" />
            <Badge variant="secondary" className="absolute bottom-2 left-2">
                <MonitorUp /> {username ? `${username} is sharing their screen` : 'Shared screen'}
            </Badge>
        </section>
    );
};
