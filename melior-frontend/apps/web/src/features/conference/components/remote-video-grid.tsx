import type { ReactNode } from 'react';
import { type Path, sharingPeers, type VoiceState } from '@/features/conference/domain';
import { cn } from '@/lib/utils';
import { RemoteVideo } from './remote-video';
import { ScreenVideo } from './screen-video';

type RemoteVideoGridProps = {
    /** One SFU stream per remote participant that has media, keyed by session id. */
    streams: Record<string, MediaStream>;
    /** Direct-link streams by session id. */
    directStreams: Record<string, MediaStream>;
    /** Screens shared over direct links, by the session id of the one sharing. */
    directScreens: Record<string, MediaStream>;
    /** The path each participant is shown over; absent means forwarded. */
    shownPaths: Record<string, Path>;
    /** Display names by session id: everyone in the room, whether their media has arrived or not. */
    usernames: Record<string, string>;
    /** Screens that participants share, by the session id of the one sharing. */
    screens: Record<string, MediaStream>;
    /** Mute and speaking per participant. */
    voices: Record<string, VoiceState>;
    /** Your own tile, shown after the others. */
    local: ReactNode;
};

/** Everyone in the room as a tile. A participant whose media has not arrived yet shows their initials. */
export const RemoteVideoGrid = ({
    streams,
    directStreams,
    directScreens,
    shownPaths,
    usernames,
    screens,
    voices,
    local,
}: RemoteVideoGridProps) => {
    const sessionIds = [
        ...new Set([...Object.keys(usernames), ...Object.keys(streams), ...Object.keys(directStreams)]),
    ];
    const alone = sessionIds.length === 0;

    const sharing = sharingPeers(screens, directScreens, shownPaths);
    const gridClass = sharing.length > 0 ? 'max-w-6xl grid-cols-2 sm:grid-cols-3 xl:grid-cols-4' : null;

    return (
        <div className="mx-auto flex w-full flex-col gap-3">
            {sharing.length > 0 && (
                <div className={cn('mx-auto grid w-full max-w-6xl gap-3', sharing.length > 1 && 'lg:grid-cols-2')}>
                    {sharing.map((sessionId) => (
                        <ScreenVideo
                            key={sessionId}
                            forwarded={screens[sessionId]}
                            direct={directScreens[sessionId]}
                            shownPath={shownPaths[sessionId] ?? 'forwarded'}
                            username={usernames[sessionId]}
                        />
                    ))}
                </div>
            )}
            <div
                className={cn(
                    'mx-auto grid w-full gap-3',
                    gridClass ?? (alone ? 'max-w-xl grid-cols-1' : 'max-w-4xl sm:grid-cols-2'),
                    !gridClass && sessionIds.length >= 4 && 'xl:grid-cols-3',
                )}
            >
                {sessionIds.map((sessionId) => (
                    <RemoteVideo
                        key={sessionId}
                        forwarded={streams[sessionId]}
                        direct={directStreams[sessionId]}
                        shownPath={shownPaths[sessionId] ?? 'forwarded'}
                        username={usernames[sessionId]}
                        voice={voices[sessionId]}
                    />
                ))}
                {local}
            </div>
        </div>
    );
};
