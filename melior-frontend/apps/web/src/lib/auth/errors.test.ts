import { describe, expect, test } from 'vitest';
import { refusalMessage, SessionRejectedError } from './errors';

describe('SessionRejectedError', () => {
    test('only the budget refusal is retryable', () => {
        expect(new SessionRejectedError('budget-exhausted').retryable).toBe(true);
        expect(new SessionRejectedError('round-trip-completed').retryable).toBe(false);
        expect(new SessionRejectedError('already-on-login').retryable).toBe(false);
    });

    test('carries the reason and its message', () => {
        const error = new SessionRejectedError('round-trip-completed');
        expect(error.reason).toBe('round-trip-completed');
        expect(error.message).toBe(refusalMessage('round-trip-completed'));
    });
});
