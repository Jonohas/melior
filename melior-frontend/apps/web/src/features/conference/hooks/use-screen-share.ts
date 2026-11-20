import { type RefObject, useCallback, useRef, useState } from 'react';
import {
    canShareScreen,
    captureScreen,
    isScreenShareCancelled,
    type PeerSession,
    stopMedia,
} from '@/features/conference/api';
import type { ConferenceConnection } from './session-messages';

export type ScreenShare = {
    /** Whether this browser can share a screen at all. */
    supported: boolean;
    /** Whether this participant is sharing its screen right now. */
    sharing: boolean;
    /** Why the last attempt to share failed; null after a success or when the person cancelled. */
    error: string | null;
    /** Asks which screen to share and starts sending it. Does nothing while sharing or not connected. */
    start: () => Promise<void>;
    stop: () => void;
    /** Offers the screen again on a new connection, so a share survives a reconnect. */
    resume: (peer: PeerSession) => void;
    /** Ends the share without touching a connection: the conference is over. */
    release: () => void;
};

/** The screen this participant shares. It outlives a connection: after a reconnect it is offered again. */
export const useScreenShare = (connection: RefObject<ConferenceConnection>): ScreenShare => {
    const screenRef = useRef<MediaStream | null>(null);
    const [sharing, setSharing] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const release = useCallback(() => {
        if (screenRef.current) stopMedia(screenRef.current);
        screenRef.current = null;
        setSharing(false);
    }, []);

    const stop = useCallback(() => {
        connection.current.peer?.stopScreen();
        release();
    }, [connection, release]);

    const start = useCallback(async () => {
        const peer = connection.current.peer;
        if (!peer || screenRef.current) return;
        setError(null);
        try {
            const screen = await captureScreen();
            // The connection may have gone while the person was choosing.
            if (connection.current.peer !== peer) {
                stopMedia(screen);
                return;
            }
            screenRef.current = screen;
            // The browser's own "Stop sharing" button ends the track.
            screen.getVideoTracks()[0]?.addEventListener('ended', () => {
                if (screenRef.current === screen) stop();
            });
            peer.shareScreen(screen);
            setSharing(true);
        } catch (failure) {
            if (isScreenShareCancelled(failure)) return;
            setError(`Could not share your screen: ${failure instanceof Error ? failure.message : String(failure)}`);
        }
    }, [connection, stop]);

    const resume = useCallback((peer: PeerSession) => {
        if (screenRef.current) peer.shareScreen(screenRef.current);
    }, []);

    return { supported: canShareScreen(), sharing, error, start, stop, resume, release };
};
