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

/** One person's tile, with the key that keeps it mounted when the layout changes. */
type Tile = { key: string; node: ReactNode };

const LOCAL_KEY = 'local';

/** Everyone in the room while nobody shares a screen: a centred grid of camera tiles. */
const CameraGrid = ({ tiles }: { tiles: Tile[] }) => {
    const remotes = tiles.length - 1;
    return (
        <div
            className={cn(
                'mx-auto grid w-full gap-3',
                remotes === 0 ? 'max-w-xl grid-cols-1' : 'max-w-4xl sm:grid-cols-2',
                remotes >= 4 && 'xl:grid-cols-3',
            )}
        >
            {tiles.map((tile) => (
                <div key={tile.key}>{tile.node}</div>
            ))}
        </div>
    );
};

/** The people in the call as a column on the left (a row on narrow screens), next to the shared screens. */
const ParticipantList = ({ tiles }: { tiles: Tile[] }) => (
    <ul
        aria-label="People in the call"
        className="flex shrink-0 gap-2 overflow-x-auto md:w-56 md:flex-col md:overflow-x-hidden md:overflow-y-auto"
    >
        {tiles.map((tile) => (
            <li key={tile.key} className="w-40 shrink-0 md:w-full">
                {tile.node}
            </li>
        ))}
    </ul>
);

/**
 * The shared screens fill the stage: one screen gets all of it, several split it into equal cells (two columns on
 * wide screens). Each cell is a size container, and its screen is the largest box of the screen's shape that
 * fits, centred both ways.
 */
const ScreenStage = ({ cells }: { cells: Tile[] }) => (
    <div className={cn('grid min-h-48 min-w-0 flex-1 auto-rows-fr gap-3', cells.length > 1 && 'lg:grid-cols-2')}>
        {cells.map((cell) => (
            <div key={cell.key} className="grid min-h-0 min-w-0 place-items-center [container-type:size]">
                {cell.node}
            </div>
        ))}
    </div>
);

/** Everyone in the room. A participant whose media has not arrived yet shows their initials. */
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
    const tiles: Tile[] = [
        ...sessionIds.map((sessionId) => ({
            key: sessionId,
            node: (
                <RemoteVideo
                    forwarded={streams[sessionId]}
                    direct={directStreams[sessionId]}
                    shownPath={shownPaths[sessionId] ?? 'forwarded'}
                    username={usernames[sessionId]}
                    voice={voices[sessionId]}
                />
            ),
        })),
        { key: LOCAL_KEY, node: local },
    ];

    const sharing = sharingPeers(screens, directScreens, shownPaths);
    if (sharing.length === 0) return <CameraGrid tiles={tiles} />;

    const cells: Tile[] = sharing.map((sessionId) => ({
        key: sessionId,
        node: (
            <ScreenVideo
                forwarded={screens[sessionId]}
                direct={directScreens[sessionId]}
                shownPath={shownPaths[sessionId] ?? 'forwarded'}
                username={usernames[sessionId]}
            />
        ),
    }));
    return (
        <div className="flex min-h-0 w-full flex-1 flex-col gap-3 md:flex-row">
            <ParticipantList tiles={tiles} />
            <ScreenStage cells={cells} />
        </div>
    );
};
