import type { Path } from '@/features/conference/domain';
import { RemoteVideo } from './remote-video';

type RemoteVideoGridProps = {
    /** One SFU stream per remote participant that has media, keyed by session id. */
    streams: Record<string, MediaStream>;
    /** Direct-link streams by session id. */
    directStreams: Record<string, MediaStream>;
    /** The path each participant is shown over; absent means forwarded. */
    shownPaths: Record<string, Path>;
    /** Display names by session id. */
    usernames: Record<string, string>;
};

/** Participants who are in the roster but have no media yet get no tile until their tracks arrive. */
export const RemoteVideoGrid = ({ streams, directStreams, shownPaths, usernames }: RemoteVideoGridProps) => {
    const sessionIds = [...new Set([...Object.keys(streams), ...Object.keys(directStreams)])];
    return (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 w-full max-w-5xl">
            {sessionIds.map((sessionId) => (
                <RemoteVideo
                    key={sessionId}
                    forwarded={streams[sessionId]}
                    direct={directStreams[sessionId]}
                    shownPath={shownPaths[sessionId] ?? 'forwarded'}
                    username={usernames[sessionId]}
                />
            ))}
        </div>
    );
};
