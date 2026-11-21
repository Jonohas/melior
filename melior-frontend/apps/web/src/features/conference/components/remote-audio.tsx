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

type PathPairProps = {
    forwarded?: MediaStream;
    direct?: MediaStream;
    /** The path this participant is heard over. */
    shown: Path;
    speakerId: string;
};

/** One sound of one participant (their microphone, or their screen's sound), with an element per path. */
const PathPairAudio = ({ forwarded, direct, shown, speakerId }: PathPairProps) => (
    <>
        {forwarded && <PathAudio stream={forwarded} audible={shown === 'forwarded'} speakerId={speakerId} />}
        {direct && <PathAudio stream={direct} audible={shown === 'direct'} speakerId={speakerId} />}
    </>
);

type RemoteAudioProps = {
    /** One SFU stream per remote participant that has media, keyed by session id. */
    streams: Record<string, MediaStream>;
    /** Direct-link streams by session id. */
    directStreams: Record<string, MediaStream>;
    /** Shared screens through the SFU, by the session id of the one sharing; they carry the screen's sound if any. */
    screens: Record<string, MediaStream>;
    /** Shared screens over direct links, by the session id of the one sharing. */
    directScreens: Record<string, MediaStream>;
    /** The path each participant is heard over; absent means forwarded. */
    shownPaths: Record<string, Path>;
    /** The speaker everyone's sound plays on; empty means the browser's default. */
    speakerId: string;
};

const sessionIdsOf = (...sources: Record<string, MediaStream>[]): string[] => [
    ...new Set(sources.flatMap((source) => Object.keys(source))),
];

/**
 * Everyone's sound in the call, without their video: the only place remote audio plays, whether the call grid is
 * on screen or not (its videos are muted). A shared screen's sound plays next to the sharer's voice.
 */
export const RemoteAudio = ({
    streams,
    directStreams,
    screens,
    directScreens,
    shownPaths,
    speakerId,
}: RemoteAudioProps) => (
    <div hidden>
        {sessionIdsOf(streams, directStreams, screens, directScreens).map((sessionId) => {
            const shown = shownPaths[sessionId] ?? 'forwarded';
            return (
                <div key={sessionId} data-session-id={sessionId}>
                    <PathPairAudio
                        forwarded={streams[sessionId]}
                        direct={directStreams[sessionId]}
                        shown={shown}
                        speakerId={speakerId}
                    />
                    <PathPairAudio
                        forwarded={screens[sessionId]}
                        direct={directScreens[sessionId]}
                        shown={shown}
                        speakerId={speakerId}
                    />
                </div>
            );
        })}
    </div>
);
