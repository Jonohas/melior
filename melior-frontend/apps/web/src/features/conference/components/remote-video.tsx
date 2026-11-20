import { useEffect, useRef } from 'react';
import { Avatar, AvatarFallback, Badge } from '@/components/ui';
import { setSpeaker } from '@/features/conference/api';

type RemoteVideoProps = {
    /** Null while the participant is in the room but their media has not arrived yet. */
    stream: MediaStream | null;
    username?: string;
    /** The speaker to play this participant's sound on; empty or missing means the browser's default. */
    speakerId?: string;
};

/** One remote participant: their video, or their initials until it arrives. */
export const RemoteVideo = ({ stream, username, speakerId }: RemoteVideoProps) => {
    const ref = useRef<HTMLVideoElement>(null);

    // The <video> only exists once the stream has arrived, so a new stream is a reason to set the speaker.
    // biome-ignore lint/correctness/useExhaustiveDependencies: see above
    useEffect(() => {
        const video = ref.current;
        if (video && speakerId !== undefined) setSpeaker(video, speakerId).catch((e) => console.warn('speaker', e));
    }, [speakerId, stream]);

    useEffect(() => {
        const video = ref.current;
        if (!video || !stream) return;
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
        <div className="relative aspect-video overflow-hidden rounded-lg border-2 border-success bg-muted">
            {stream ? (
                // biome-ignore lint/a11y/useMediaCaption: live remote stream has no captions
                <video ref={ref} autoPlay playsInline className="size-full bg-black object-contain" />
            ) : (
                <div className="grid size-full place-items-center">
                    <Avatar className="size-16">
                        <AvatarFallback className="bg-primary/15 text-xl text-primary">
                            {(username ?? '?').slice(0, 2).toUpperCase()}
                        </AvatarFallback>
                    </Avatar>
                </div>
            )}
            {username && (
                <Badge variant="secondary" className="absolute bottom-2 left-2">
                    {username}
                </Badge>
            )}
        </div>
    );
};
