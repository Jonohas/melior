/**
 * How sharp and smooth your video is: your choice, not the app's. `best` asks the camera or the screen capture
 * for the most it offers; anything else caps it. Bandwidth is never capped here: the browser's congestion
 * control fits what is sent to the network.
 */

export type CameraQuality = 'best' | '1080p' | '720p' | '480p' | '360p';
export type ScreenFrameRate = 'best' | '60' | '30' | '15' | '5';

export type VideoQuality = { camera: CameraQuality; screenFrameRate: ScreenFrameRate };

export const CAMERA_QUALITIES: readonly CameraQuality[] = ['best', '1080p', '720p', '480p', '360p'];
export const SCREEN_FRAME_RATES: readonly ScreenFrameRate[] = ['best', '60', '30', '15', '5'];

export const DEFAULT_VIDEO_QUALITY: VideoQuality = { camera: 'best', screenFrameRate: 'best' };

/** The largest capture a camera or a screen is asked for when nothing is capped: 4K at 60 frames per second. */
const BEST: MediaTrackConstraints = { width: { ideal: 3840 }, height: { ideal: 2160 }, frameRate: { ideal: 60 } };

const CAMERA_HEIGHT: Record<Exclude<CameraQuality, 'best'>, number> = {
    '1080p': 1080,
    '720p': 720,
    '480p': 480,
    '360p': 360,
};

/** What the camera is asked for. A cap names the largest height; the width follows the camera's aspect. */
export const cameraConstraints = (quality: CameraQuality): MediaTrackConstraints => {
    if (quality === 'best') return BEST;
    const height = CAMERA_HEIGHT[quality];
    return { height: { ideal: height, max: height }, frameRate: { ideal: 30 } };
};

/** What a screen capture is asked for. A screen is captured at its own resolution; only its frame rate is chosen. */
export const screenConstraints = (rate: ScreenFrameRate): MediaTrackConstraints => {
    if (rate === 'best') return { frameRate: BEST.frameRate };
    const frames = Number(rate);
    return { frameRate: { ideal: frames, max: frames } };
};

const pick = <T extends string>(value: unknown, allowed: readonly T[], fallback: T): T =>
    allowed.find((known) => known === value) ?? fallback;

/** Reads the stored choice. Anything missing or unknown falls back to `best`, never to a surprise cap. */
export const parseVideoQuality = (raw: string | null): VideoQuality => {
    if (!raw) return DEFAULT_VIDEO_QUALITY;
    let value: unknown;
    try {
        value = JSON.parse(raw);
    } catch {
        return DEFAULT_VIDEO_QUALITY;
    }
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return DEFAULT_VIDEO_QUALITY;
    const { camera, screenFrameRate } = value as Record<string, unknown>;
    return {
        camera: pick(camera, CAMERA_QUALITIES, 'best'),
        screenFrameRate: pick(screenFrameRate, SCREEN_FRAME_RATES, 'best'),
    };
};
