import { useEffect, useRef } from 'react';

type LocalPreviewProps = {
    stream: MediaStream | null;
    username: string | null;
};

export const LocalPreview = ({ stream, username }: LocalPreviewProps) => {
    const ref = useRef<HTMLVideoElement>(null);

    useEffect(() => {
        if (ref.current) ref.current.srcObject = stream;
    }, [stream]);

    return (
        <div>
            <p>{username ? `${username} (you)` : 'Local (your camera)'}</p>
            <video
                ref={ref}
                autoPlay
                muted
                playsInline
                className="w-[400px] max-w-full aspect-video bg-black"
                style={{ transform: 'scaleX(-1)' }}
            />
        </div>
    );
};
