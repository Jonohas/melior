/** RMS level (0..1) of the microphone above which the person counts as speaking. */
export const SPEAKING_THRESHOLD = 0.04;

/** How long the person still counts as speaking after the level dropped below the threshold. */
export const SPEAKING_HOLD_MS = 300;

/** How often the microphone level is sampled. */
export const SPEAKING_SAMPLE_MS = 50;

export type SpeakingState = {
    speaking: boolean;
    /** When the level was last above the threshold, or null if never. */
    lastLoudMs: number | null;
};

export const QUIET: SpeakingState = { speaking: false, lastLoudMs: null };

/**
 * One level sample: loud makes the person speak at once, quiet ends it only after the hold, so the pauses
 * between words do not flicker. A muted microphone is never speaking.
 */
export const nextSpeaking = (state: SpeakingState, level: number, nowMs: number, muted: boolean): SpeakingState => {
    if (muted) return QUIET;
    if (level >= SPEAKING_THRESHOLD) return { speaking: true, lastLoudMs: nowMs };
    if (state.lastLoudMs === null) return QUIET;
    const held = nowMs - state.lastLoudMs < SPEAKING_HOLD_MS;
    return { speaking: held, lastLoudMs: state.lastLoudMs };
};

/** Root mean square of time-domain samples in [-1, 1]; 0 for no samples. */
export const rmsLevel = (samples: ArrayLike<number>): number => {
    if (samples.length === 0) return 0;
    let sum = 0;
    for (let i = 0; i < samples.length; i++) sum += (samples[i] ?? 0) * (samples[i] ?? 0);
    return Math.sqrt(sum / samples.length);
};
