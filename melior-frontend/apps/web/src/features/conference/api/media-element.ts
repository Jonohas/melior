/**
 * Plays a remote stream on a media element and keeps it playing. A reused media section hands a participant a
 * track that lived in another stream before; the element is paused after its previous tracks ended, so it is
 * nudged whenever the track set changes. Returns the function that stops watching.
 */
export const playStream = (element: HTMLMediaElement, stream: MediaStream): (() => void) => {
    element.srcObject = stream;
    const nudge = () => void element.play().catch(() => undefined);
    stream.addEventListener('addtrack', nudge);
    stream.addEventListener('removetrack', nudge);
    nudge();
    return () => {
        stream.removeEventListener('addtrack', nudge);
        stream.removeEventListener('removetrack', nudge);
    };
};

/**
 * Calls `onSize` with a video's frame size now and whenever it changes (a sharer who resizes the shared window,
 * or the first frame arriving). Returns the function that stops watching.
 */
export const watchVideoSize = (
    video: HTMLVideoElement,
    onSize: (width: number, height: number) => void,
): (() => void) => {
    const report = () => onSize(video.videoWidth, video.videoHeight);
    video.addEventListener('loadedmetadata', report);
    video.addEventListener('resize', report);
    report();
    return () => {
        video.removeEventListener('loadedmetadata', report);
        video.removeEventListener('resize', report);
    };
};
