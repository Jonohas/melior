import { useEffect, useRef } from 'react';
import { playStream, setSpeaker } from '@/features/conference/api';
import type { Path } from '@/features/conference/domain';

type PathAudioProps = {
    stream: MediaStream;
    /** Only the shown path is heard; the other keeps playing silently, so a switch is instant. */
    audible: boolean;
    speakerId: string;
};

const PathAudio = ({ stream, audible, speakerId }: PathAudioProps) => {
    const ref = useRef<HTMLAudioElement>(null);

    useEffect(() => {
        const audio = ref.current;
        if (!audio) return;
        return playStream(audio, stream);
    }, [stream]);

    useEffect(() => {
        const audio = ref.current;
        if (audio) setSpeaker(audio, speakerId).catch((error) => console.warn('speaker', error));
    }, [speakerId]);

    return <audio ref={ref} autoPlay muted={!audible} />;
};

type RemoteAudioProps = {
    /** One SFU stream per remote participant that has media, keyed by session id. */
    streams: Record<string, MediaStream>;
    /** Direct-link streams by session id. */
    directStreams: Record<string, MediaStream>;
    /** The path each participant is heard over; absent means forwarded. */
    shownPaths: Record<string, Path>;
    /** The speaker everyone's sound plays on; empty means the browser's default. */
    speakerId: string;
};

/**
 * Everyone's sound in the call, without their video: the only place remote audio plays, whether the call grid is
 * on screen or not (its videos are muted).
 */
export const RemoteAudio = ({ streams, directStreams, shownPaths, speakerId }: RemoteAudioProps) => {
    const sessionIds = [...new Set([...Object.keys(streams), ...Object.keys(directStreams)])];
    return (
        <div hidden>
            {sessionIds.map((sessionId) => {
                const shown = shownPaths[sessionId] ?? 'forwarded';
                const forwarded = streams[sessionId];
                const direct = directStreams[sessionId];
                return (
                    <div key={sessionId} data-session-id={sessionId}>
                        {forwarded && (
                            <PathAudio stream={forwarded} audible={shown === 'forwarded'} speakerId={speakerId} />
                        )}
                        {direct && <PathAudio stream={direct} audible={shown === 'direct'} speakerId={speakerId} />}
                    </div>
                );
            })}
        </div>
    );
};
