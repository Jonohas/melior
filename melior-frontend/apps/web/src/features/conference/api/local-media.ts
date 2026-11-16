/**
 * Microphone capture. Remote audio is played by the remote <video> elements; the local preview stays muted.
 * Two tabs on one machine will feed back into each other unless you use headphones.
 */
export const ENABLE_AUDIO = true;

const AUDIO_CONSTRAINTS: MediaTrackConstraints = {
    echoCancellation: true,
    noiseSuppression: true,
    autoGainControl: true,
};

export const captureLocalMedia = (): Promise<MediaStream> =>
    navigator.mediaDevices.getUserMedia({ video: true, audio: ENABLE_AUDIO ? AUDIO_CONSTRAINTS : false });

export const stopMedia = (stream: MediaStream): void => {
    for (const track of stream.getTracks()) track.stop();
};
