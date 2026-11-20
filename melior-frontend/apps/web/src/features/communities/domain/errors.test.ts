import { describe, expect, test } from 'vitest';
import { CommunityRequestError, failureOfStatus, isRequestFailure } from './errors';

describe('failureOfStatus', () => {
    test('maps the documented statuses', () => {
        expect([400, 403, 404, 409, 410, 503, 500].map(failureOfStatus)).toEqual([
            'invalid',
            'forbidden',
            'not_found',
            'conflict',
            'gone',
            'unavailable',
            'failed',
        ]);
    });
});

describe('CommunityRequestError', () => {
    test("prefers the server's reason and falls back to a default", () => {
        expect(new CommunityRequestError(400, 'name too long').message).toBe('name too long');
        expect(new CommunityRequestError(410, null).message).toBe('This invite is no longer valid.');
        expect(isRequestFailure(new CommunityRequestError(404, null), 'not_found')).toBe(true);
        expect(isRequestFailure(new Error('x'), 'not_found')).toBe(false);
    });
});
