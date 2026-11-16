import { useEffect, useRef } from 'react';

type RemoteVideoProps = {
    stream: MediaStream;
    username?: string;
};

export const RemoteVideo = ({ stream, username }: RemoteVideoProps) => {
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
        <div className="border-2 rounded border-green-500">
            {/* biome-ignore lint/a11y/useMediaCaption: live remote stream has no captions */}
            <video ref={ref} autoPlay playsInline className="w-full aspect-video bg-black" />
            {username && <p className="px-2 py-1">{username}</p>}
        </div>
    );
};
