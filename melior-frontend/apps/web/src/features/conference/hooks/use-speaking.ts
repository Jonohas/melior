import { useEffect, useRef, useState } from 'react';
import { createLevelMeter } from '@/features/conference/api';
import { nextSpeaking, QUIET, SPEAKING_SAMPLE_MS } from '@/features/conference/domain';

/**
 * Whether the person is speaking into their own microphone right now, from its level (threshold plus a short
 * hold, see the domain's `nextSpeaking`). `audioDeviceId` restarts the meter after a microphone switch, which
 * replaces the stream's audio track.
 */
export const useSpeaking = (stream: MediaStream | null, audioDeviceId: string | null, muted: boolean): boolean => {
    const [speaking, setSpeaking] = useState(false);
    const mutedRef = useRef(muted);
    useEffect(() => {
        mutedRef.current = muted;
    });

    // biome-ignore lint/correctness/useExhaustiveDependencies: a new microphone is a new track to measure
    useEffect(() => {
        if (!stream) return;
        const meter = createLevelMeter(stream);
        if (!meter) return;
        let state = QUIET;
        const timer = setInterval(() => {
            state = nextSpeaking(state, meter.level(), performance.now(), mutedRef.current);
            setSpeaking(state.speaking);
        }, SPEAKING_SAMPLE_MS);
        return () => {
            clearInterval(timer);
            meter.close();
            setSpeaking(false);
        };
    }, [stream, audioDeviceId]);

    return speaking && !muted;
};
