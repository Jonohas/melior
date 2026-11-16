import { RemoteVideo } from './remote-video';

type RemoteVideoGridProps = {
    /** One stream per remote participant that has media, keyed by session id. */
    streams: Record<string, MediaStream>;
    /** Display names by session id. */
    usernames: Record<string, string>;
};

/** Participants who are in the roster but have no media yet get no tile until their tracks arrive. */
export const RemoteVideoGrid = ({ streams, usernames }: RemoteVideoGridProps) => {
    return (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 w-full max-w-5xl">
            {Object.entries(streams).map(([sessionId, stream]) => (
                <RemoteVideo key={sessionId} stream={stream} username={usernames[sessionId]} />
            ))}
        </div>
    );
};
