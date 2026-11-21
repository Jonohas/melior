import { describe, expect, test } from 'vitest';
import { cameraConstraints, DEFAULT_VIDEO_QUALITY, parseVideoQuality, screenConstraints } from './video-quality';

describe('video quality', () => {
    test('best caps nothing: the camera is asked for the most it offers', () => {
        const best = cameraConstraints('best');
        expect(JSON.stringify(best)).not.toContain('max');
        expect(best.height).toEqual({ ideal: 2160 });
    });

    test('a chosen camera quality caps the height', () => {
        expect(cameraConstraints('720p').height).toEqual({ ideal: 720, max: 720 });
        expect(cameraConstraints('360p').height).toEqual({ ideal: 360, max: 360 });
    });

    test('a screen is capped only in frame rate, and only when chosen', () => {
        expect(screenConstraints('best')).toEqual({ frameRate: { ideal: 60 } });
        expect(screenConstraints('15')).toEqual({ frameRate: { ideal: 15, max: 15 } });
    });

    test('a stored choice is read back; anything else means best', () => {
        expect(parseVideoQuality(JSON.stringify({ camera: '480p', screenFrameRate: '30' }))).toEqual({
            camera: '480p',
            screenFrameRate: '30',
        });
        expect(parseVideoQuality(JSON.stringify({ camera: '8k', screenFrameRate: 30 }))).toEqual(DEFAULT_VIDEO_QUALITY);
        expect(parseVideoQuality('not json')).toEqual(DEFAULT_VIDEO_QUALITY);
        expect(parseVideoQuality(null)).toEqual(DEFAULT_VIDEO_QUALITY);
    });
});
