/** The shape a shared screen is shown in until its first frame says otherwise: most monitors are 16:9. */
export const DEFAULT_SCREEN_ASPECT = 16 / 9;

/**
 * The width-to-height ratio of a video frame, or null while the size is unknown (a video reports 0 × 0 before
 * its first frame) or makes no sense.
 */
export const aspectOf = (width: number, height: number): number | null => {
    if (!Number.isFinite(width) || !Number.isFinite(height)) return null;
    if (width <= 0 || height <= 0) return null;
    return width / height;
};
