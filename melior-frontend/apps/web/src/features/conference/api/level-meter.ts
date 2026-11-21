import { rmsLevel } from '@/features/conference/domain';

/** The loudness of a microphone, sampled on demand. */
export type LevelMeter = {
    /** RMS level of the latest audio frame, 0..1. */
    level: () => number;
    close: () => void;
};

const FFT_SIZE = 512;

/**
 * Measures the stream's first audio track with a Web Audio `AnalyserNode`. Null when the stream has no audio
 * or the browser has no Web Audio. The node is not connected to the speakers, so nothing is heard.
 */
export const createLevelMeter = (stream: MediaStream): LevelMeter | null => {
    const track = stream.getAudioTracks()[0];
    if (!track || typeof AudioContext === 'undefined') return null;
    const context = new AudioContext();
    const source = context.createMediaStreamSource(new MediaStream([track]));
    const analyser = context.createAnalyser();
    analyser.fftSize = FFT_SIZE;
    source.connect(analyser);
    const samples = new Float32Array(analyser.fftSize);
    return {
        level: () => {
            // Autoplay rules may start the context suspended; resuming is harmless once it runs.
            if (context.state === 'suspended') void context.resume().catch(() => undefined);
            analyser.getFloatTimeDomainData(samples);
            return rmsLevel(samples);
        },
        close: () => {
            source.disconnect();
            void context.close().catch(() => undefined);
        },
    };
};
