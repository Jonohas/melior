import { type RefObject, useCallback, useEffect, useMemo, useState } from 'react';
import { type CallRecorder, createCallRecorder } from '@/features/conference/api';
import type { CallDebugSnapshot } from '@/features/conference/domain';
import type { ConferenceConnection } from './session-messages';

/** How often the debug panel reads the connections' stats while it is shown. */
export const DEBUG_REFRESH_MS = 1000;

/** What the debug panel reads from a call. */
export type CallDebug = {
    /** Reads every connection's stats once; only the panel calls it, so a call without it polls nothing. */
    sample: () => Promise<CallDebugSnapshot>;
};

/** The call's recorder, and the debug handle that samples the connections the call has right now. */
export const useCallRecorder = (
    connection: RefObject<ConferenceConnection>,
    forceRelay: boolean,
): { recorder: CallRecorder; debug: CallDebug } => {
    const [recorder] = useState(() => createCallRecorder());
    const sample = useCallback(() => {
        const { peer, mesh } = connection.current;
        return recorder.sample({
            forceRelay,
            sfu: peer?.statsSource.connection ?? null,
            links: mesh?.debugLinks() ?? [],
        });
    }, [recorder, connection, forceRelay]);
    const debug = useMemo(() => ({ sample }), [sample]);
    return { recorder, debug };
};

/** Samples the call about once a second while mounted; null until the first sample is in. */
export const useCallDebug = (debug: CallDebug): CallDebugSnapshot | null => {
    const [snapshot, setSnapshot] = useState<CallDebugSnapshot | null>(null);
    useEffect(() => {
        let stopped = false;
        let busy = false;
        const refresh = async () => {
            if (busy) return;
            busy = true;
            try {
                const next = await debug.sample();
                if (!stopped) setSnapshot(next);
            } catch (error) {
                console.warn('debug panel: sampling failed', error);
            } finally {
                busy = false;
            }
        };
        void refresh();
        const timer = setInterval(() => void refresh(), DEBUG_REFRESH_MS);
        return () => {
            stopped = true;
            clearInterval(timer);
        };
    }, [debug]);
    return snapshot;
};
