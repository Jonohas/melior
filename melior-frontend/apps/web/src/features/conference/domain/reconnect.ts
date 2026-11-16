import type { CloseReason } from './signalling-message';

export const RECONNECT_BASE_MS = 1000;
export const RECONNECT_MAX_MS = 15_000;

/**
 * Delay before reconnect attempt `attempt` (1-based): 1 s doubling to a 15 s cap, with "equal jitter"
 * (half fixed, half random) so many clients that lost the same server do not return in lockstep. The cap
 * is never exceeded. `random` is in [0, 1).
 */
export const reconnectDelayMs = (attempt: number, random: number): number => {
    const exponent = Math.max(0, Math.min(attempt - 1, 30));
    const ceiling = Math.min(RECONNECT_MAX_MS, RECONNECT_BASE_MS * 2 ** exponent);
    return Math.round(ceiling / 2 + (ceiling / 2) * Math.min(Math.max(random, 0), 1));
};

/** The attempt number for the next reconnect: a connection that reached `connected` restarts the backoff. */
export const nextAttempt = (previousAttempt: number, reachedConnected: boolean): number =>
    reachedConnected ? 1 : previousAttempt + 1;

/** Why the conference gave up for good. */
export type StopReason = 'removed' | 'not_invited' | 'no_access' | 'unauthorized';

export const describeStop = (reason: StopReason): string => {
    switch (reason) {
        case 'removed':
            return 'You were removed from this room';
        case 'not_invited':
            return 'You are not invited to this room';
        case 'no_access':
            return 'This account has no access to Melior';
        case 'unauthorized':
            return 'Your session has ended';
    }
};

export type CloseDecision =
    | { kind: 'reconnect'; immediate: boolean }
    | { kind: 'stop'; reason: StopReason }
    /** Ask the server whether the session (and, after a refused upgrade, the room) is still fine. */
    | { kind: 'diagnose'; checkRoom: boolean };

/**
 * What to do when the signalling socket is gone. `serverReason` is the `closed` message the server sent
 * right before closing, if any; `opened` is whether the socket ever opened.
 */
export const decideOnClose = ({
    serverReason,
    opened,
}: {
    serverReason: CloseReason | null;
    opened: boolean;
}): CloseDecision => {
    switch (serverReason) {
        case 'session_ended':
            return { kind: 'diagnose', checkRoom: false };
        case 'removed':
            return { kind: 'stop', reason: 'removed' };
        case 'room_moved':
            return { kind: 'reconnect', immediate: true };
        case null:
            // The browser reports no status for a refused upgrade, so a close before open could be 401, 403 or 503.
            return opened ? { kind: 'reconnect', immediate: false } : { kind: 'diagnose', checkRoom: true };
    }
};

export type SessionProbe = 'ok' | 'unauthorized' | 'forbidden' | 'error';
export type RoomProbe = 'ok' | 'forbidden' | 'unknown';

/** What to do once the session (and room) were re-checked. Only 401, 403 and "not invited" are final. */
export const decideAfterDiagnosis = (session: SessionProbe, room: RoomProbe): CloseDecision => {
    if (session === 'unauthorized') return { kind: 'stop', reason: 'unauthorized' };
    if (session === 'forbidden') return { kind: 'stop', reason: 'no_access' };
    if (session === 'ok' && room === 'forbidden') return { kind: 'stop', reason: 'not_invited' };
    return { kind: 'reconnect', immediate: false };
};
