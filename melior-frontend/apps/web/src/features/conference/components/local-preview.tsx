import { MicOff, VideoOff } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { Badge } from '@/components/ui';
import { useFullscreen } from '@/features/conference/hooks';
import { cn } from '@/lib/utils';
import { FULLSCREEN_TILE, FullscreenButton } from './fullscreen-button';

type LocalPreviewProps = {
    stream: MediaStream | null;
    username: string | null;
    /** Shown on the tile, so you can tell the others cannot hear you. */
    microphoneMuted?: boolean;
    cameraOff?: boolean;
    /** Your microphone picks up speech: the tile gets the speaking ring. */
    speaking?: boolean;
};

/** Your own camera, mirrored and muted. A double click or the corner button shows it fullscreen. */
export const LocalPreview = ({
    stream,
    username,
    microphoneMuted = false,
    cameraOff = false,
    speaking = false,
}: LocalPreviewProps) => {
    const ref = useRef<HTMLVideoElement>(null);
    const tileRef = useRef<HTMLDivElement>(null);
    const fullscreen = useFullscreen(tileRef);

    useEffect(() => {
        if (ref.current) ref.current.srcObject = stream;
    }, [stream]);

    return (
        <div
            ref={tileRef}
            data-speaking={speaking}
            onDoubleClick={fullscreen.toggle}
            className={cn(
                'relative aspect-video overflow-hidden rounded-lg border-2 border-border bg-muted transition-colors',
                FULLSCREEN_TILE,
                speaking && 'border-success',
            )}
        >
            <video ref={ref} autoPlay muted playsInline className="size-full -scale-x-100 bg-black object-contain" />
            {cameraOff && (
                <div className="absolute inset-0 grid place-items-center bg-muted text-sm text-muted-foreground">
                    <span className="flex items-center gap-2">
                        <VideoOff className="size-4" aria-hidden="true" /> Your camera is off
                    </span>
                </div>
            )}
            {microphoneMuted && (
                <Badge variant="destructive" className="absolute top-2 left-2">
                    <MicOff /> Muted
                </Badge>
            )}
            <Badge variant="secondary" className="absolute bottom-2 left-2">
                {username ? `${username} (you)` : 'You'}
            </Badge>
            {!cameraOff && <FullscreenButton fullscreen={fullscreen} what="your camera" />}
        </div>
    );
};
