type PeerScreen = {
    /** The peer says whether it shares its screen. */
    setSharing: (sharing: boolean) => void;
    /** The screen section delivered its track. */
    trackArrived: (track: MediaStreamTrack) => void;
};

/**
 * The peer's screen on a direct link: shown only while the peer says it shares one and its section has
 * delivered a track. `onScreen` hears of every change once: the stream, or null.
 */
export const createPeerScreen = (onScreen: (stream: MediaStream | null) => void): PeerScreen => {
    const stream = new MediaStream();
    let sharing = false;
    let arrived = false;
    let published: MediaStream | null = null;

    const publish = () => {
        const next = sharing && arrived ? stream : null;
        if (next === published) return;
        published = next;
        onScreen(next);
    };

    return {
        setSharing: (value) => {
            sharing = value;
            publish();
        },
        trackArrived: (track) => {
            stream.addTrack(track);
            arrived = true;
            publish();
        },
    };
};
