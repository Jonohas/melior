import { describe, expect, test } from 'vitest';
import {
    decideAfterDiagnosis,
    decideOnClose,
    describeStop,
    nextAttempt,
    RECONNECT_MAX_MS,
    reconnectDelayMs,
} from './reconnect';

describe('reconnectDelayMs', () => {
    test('doubles from one second, with jitter between half and full of the step', () => {
        expect(reconnectDelayMs(1, 0)).toBe(500);
        expect(reconnectDelayMs(1, 0.999)).toBeLessThanOrEqual(1000);
        expect(reconnectDelayMs(2, 0)).toBe(1000);
        expect(reconnectDelayMs(3, 0)).toBe(2000);
        expect(reconnectDelayMs(4, 0)).toBe(4000);
        expect(reconnectDelayMs(5, 0)).toBe(7500); // 16 s step, capped at 15 s
    });

    test('never exceeds the cap, however many attempts', () => {
        for (const attempt of [5, 6, 7, 20, 100, 10_000]) {
            expect(reconnectDelayMs(attempt, 0.999999)).toBeLessThanOrEqual(RECONNECT_MAX_MS);
        }
        expect(reconnectDelayMs(100, 1)).toBe(RECONNECT_MAX_MS);
        expect(reconnectDelayMs(100, 0)).toBe(RECONNECT_MAX_MS / 2);
    });

    test('tolerates out-of-range input', () => {
        expect(reconnectDelayMs(0, 0)).toBe(500);
        expect(reconnectDelayMs(1, -5)).toBe(500);
        expect(reconnectDelayMs(1, 5)).toBe(1000);
    });
});

describe('nextAttempt', () => {
    test('counts consecutive failures and restarts after a connection that reached connected', () => {
        expect(nextAttempt(0, false)).toBe(1);
        expect(nextAttempt(3, false)).toBe(4);
        expect(nextAttempt(7, true)).toBe(1);
    });
});

describe('decideOnClose', () => {
    test('session_ended re-checks the session only', () => {
        expect(decideOnClose({ serverReason: 'session_ended', opened: true })).toEqual({
            kind: 'diagnose',
            checkChannel: false,
        });
    });

    test('removed stops without retry, room_moved reconnects immediately', () => {
        expect(decideOnClose({ serverReason: 'removed', opened: true })).toEqual({ kind: 'stop', reason: 'removed' });
        expect(decideOnClose({ serverReason: 'room_moved', opened: true })).toEqual({
            kind: 'reconnect',
            immediate: true,
        });
    });

    test('an unexpected loss after open reconnects with backoff', () => {
        expect(decideOnClose({ serverReason: null, opened: true })).toEqual({ kind: 'reconnect', immediate: false });
    });

    test('a close before open diagnoses session and room', () => {
        expect(decideOnClose({ serverReason: null, opened: false })).toEqual({ kind: 'diagnose', checkChannel: true });
    });
});

describe('decideAfterDiagnosis', () => {
    test('401 and 403 are final', () => {
        expect(decideAfterDiagnosis('unauthorized', 'unknown')).toEqual({ kind: 'stop', reason: 'unauthorized' });
        expect(decideAfterDiagnosis('forbidden', 'unknown')).toEqual({ kind: 'stop', reason: 'no_access' });
    });

    test('a refused, unknown or full channel is final only when the session is fine', () => {
        expect(decideAfterDiagnosis('ok', 'forbidden')).toEqual({ kind: 'stop', reason: 'removed' });
        expect(decideAfterDiagnosis('ok', 'not_found')).toEqual({ kind: 'stop', reason: 'channel_not_found' });
        expect(decideAfterDiagnosis('ok', 'full')).toEqual({ kind: 'stop', reason: 'channel_full' });
        expect(decideAfterDiagnosis('error', 'forbidden')).toEqual({ kind: 'reconnect', immediate: false });
        expect(decideAfterDiagnosis('error', 'full')).toEqual({ kind: 'reconnect', immediate: false });
    });

    test('everything else retries with backoff', () => {
        expect(decideAfterDiagnosis('ok', 'ok')).toEqual({ kind: 'reconnect', immediate: false });
        expect(decideAfterDiagnosis('ok', 'unknown')).toEqual({ kind: 'reconnect', immediate: false });
        expect(decideAfterDiagnosis('error', 'unknown')).toEqual({ kind: 'reconnect', immediate: false });
    });
});

describe('describeStop', () => {
    test('has the user-facing texts', () => {
        expect(describeStop('removed')).toBe('You were removed from this channel');
        expect(describeStop('channel_not_found')).toBe('This channel does not exist');
        expect(describeStop('channel_full')).toBe('This channel is full');
    });
});
