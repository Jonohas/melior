import type { Dispatch } from 'react';
import {
    type CloseDecision,
    type CloseReason,
    type ConferenceAction,
    decideAfterDiagnosis,
    decideOnClose,
    nextAttempt,
    type RoomProbe,
    reconnectDelayMs,
    type SessionProbe,
    type StopReason,
} from '@/features/conference/domain';

type Check<T> = { current: (() => Promise<T>) | undefined };

type ReconnectorDeps = {
    dispatch: Dispatch<ConferenceAction<MediaStream>>;
    isDisposed: () => boolean;
    /** Bumped on every teardown, so late callbacks of an old connection are ignored. */
    generation: () => number;
    /** Bumps the generation and closes everything the connection owns. */
    teardown: () => void;
    /** Closes everything the connection owns, keeping the generation. */
    release: () => void;
    connect: () => void;
    checkSession: Check<SessionProbe>;
    checkRoomAccess: Check<RoomProbe>;
};

export type Reconnector = {
    /** The socket closed or the server said `closed`: decide what happens next and do it. */
    onClosed: (serverReason: CloseReason | null, opened: boolean) => void;
    scheduleReconnect: (immediate: boolean) => void;
    /** The peer connection reached `connected`. */
    markConnected: () => void;
    dispose: () => void;
};

const probe = async <T>(check: (() => Promise<T>) | undefined, fallback: T): Promise<T> => {
    try {
        return check ? await check() : fallback;
    } catch (error) {
        console.warn('re-check failed', error);
        return fallback;
    }
};

/**
 * Executes the pure decisions of the domain after a lost connection: reconnect after a delay, stop for good,
 * or re-check the login session and the room access first. Owns the attempt counter and the retry timer.
 */
export const createReconnector = (deps: ReconnectorDeps): Reconnector => {
    const { dispatch, isDisposed, generation, teardown, release, connect } = deps;
    let timer: ReturnType<typeof setTimeout> | null = null;
    let attempt = 0;
    let reachedConnected = false;

    const stop = (reason: StopReason) => {
        teardown();
        dispatch({ type: 'stopped', reason });
    };

    const scheduleReconnect = (immediate: boolean) => {
        if (isDisposed()) return;
        teardown();
        attempt = nextAttempt(attempt, reachedConnected);
        reachedConnected = false;
        dispatch({ type: 'reconnecting', attempt });
        const delay = immediate ? 0 : reconnectDelayMs(attempt, Math.random());
        if (timer) clearTimeout(timer);
        timer = setTimeout(() => {
            timer = null;
            connect();
        }, delay);
    };

    const diagnose = async (checkRoom: boolean) => {
        const mine = generation();
        const session = await probe<SessionProbe>(deps.checkSession.current, 'error');
        const room =
            session === 'ok' && checkRoom ? await probe<RoomProbe>(deps.checkRoomAccess.current, 'unknown') : 'unknown';
        if (isDisposed() || mine !== generation()) return;
        execute(decideAfterDiagnosis(session, room));
    };

    const execute = (decision: CloseDecision) => {
        if (isDisposed()) return;
        switch (decision.kind) {
            case 'reconnect':
                scheduleReconnect(decision.immediate);
                break;
            case 'stop':
                stop(decision.reason);
                break;
            case 'diagnose':
                void diagnose(decision.checkRoom);
                break;
        }
    };

    return {
        scheduleReconnect,
        onClosed: (serverReason, opened) => {
            if (isDisposed()) return;
            const decision = decideOnClose({ serverReason, opened });
            // The generation stays, so a diagnosis started now is still current when it finishes.
            release();
            execute(decision);
        },
        markConnected: () => {
            reachedConnected = true;
        },
        dispose: () => {
            if (timer) clearTimeout(timer);
            timer = null;
        },
    };
};
