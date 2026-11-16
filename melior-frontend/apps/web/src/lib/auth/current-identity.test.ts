import { afterEach, describe, expect, test, vi } from 'vitest';
import { getCurrentIdentity, parseIdentity } from './current-identity';
import { ForbiddenError, UnauthorizedError } from './errors';

const body = { subject: 'sub-1', username: 'alice', email: 'a@example.test', roles: ['member'] };

describe('parseIdentity', () => {
    test('accepts the documented shape', () => {
        expect(parseIdentity(body)).toEqual(body);
    });

    test('treats a missing email as null', () => {
        expect(parseIdentity({ ...body, email: undefined })?.email).toBeNull();
        expect(parseIdentity({ ...body, email: null })?.email).toBeNull();
    });

    test('keeps an empty roles list', () => {
        expect(parseIdentity({ ...body, roles: [] })?.roles).toEqual([]);
    });

    test('rejects malformed bodies', () => {
        expect(parseIdentity(null)).toBeNull();
        expect(parseIdentity('x')).toBeNull();
        expect(parseIdentity({ ...body, subject: 1 })).toBeNull();
        expect(parseIdentity({ ...body, username: undefined })).toBeNull();
        expect(parseIdentity({ ...body, email: 5 })).toBeNull();
        expect(parseIdentity({ ...body, roles: 'member' })).toBeNull();
        expect(parseIdentity({ ...body, roles: [1] })).toBeNull();
    });
});

describe('getCurrentIdentity', () => {
    afterEach(() => vi.unstubAllGlobals());

    const respond = (status: number, json?: unknown) =>
        vi.stubGlobal(
            'fetch',
            vi.fn(async () => new Response(json === undefined ? null : JSON.stringify(json), { status })),
        );

    test('returns the identity on 200 and fetches the plain same-origin URL', async () => {
        respond(200, body);
        await expect(getCurrentIdentity()).resolves.toEqual(body);
        expect(vi.mocked(fetch).mock.calls[0]).toEqual(['/api/identity/me']);
    });

    test('maps 401 and 403 to typed errors', async () => {
        respond(401);
        await expect(getCurrentIdentity()).rejects.toBeInstanceOf(UnauthorizedError);
        respond(403);
        await expect(getCurrentIdentity()).rejects.toBeInstanceOf(ForbiddenError);
    });

    test('other failures are plain errors, not a session expiry', async () => {
        respond(404);
        const error = await getCurrentIdentity().catch((e: unknown) => e);
        expect(error).toBeInstanceOf(Error);
        expect(error).not.toBeInstanceOf(UnauthorizedError);
        respond(200, { nope: true });
        await expect(getCurrentIdentity()).rejects.toThrow('Unexpected response');
    });
});
