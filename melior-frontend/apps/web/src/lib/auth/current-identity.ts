import { ForbiddenError, UnauthorizedError } from './errors';

/** The caller as `GET /api/identity/me` reports it. */
export type CurrentIdentity = {
    subject: string;
    username: string;
    email: string | null;
    roles: string[];
};

/** Validates the `/api/identity/me` body. Returns null for anything that is not the documented shape. */
export const parseIdentity = (raw: unknown): CurrentIdentity | null => {
    if (typeof raw !== 'object' || raw === null) return null;
    const { subject, username, email, roles } = raw as Record<string, unknown>;
    if (typeof subject !== 'string' || typeof username !== 'string') return null;
    if (email !== undefined && email !== null && typeof email !== 'string') return null;
    if (!Array.isArray(roles) || !roles.every((role) => typeof role === 'string')) return null;
    return { subject, username, email: email ?? null, roles };
};

/**
 * Plain same-origin fetch: the session cookie rides along. 401 means not signed in, 403 means signed in
 * without a Melior role. Other failures are plain errors, deliberately not turned into 401 (a missing
 * endpoint is not an expired session, and treating it as one would loop through login).
 */
export const getCurrentIdentity = async (): Promise<CurrentIdentity> => {
    const response = await fetch('/api/identity/me');
    if (response.status === 401) throw new UnauthorizedError();
    if (response.status === 403) throw new ForbiddenError();
    if (!response.ok) throw new Error(`Loading the current identity failed with HTTP ${response.status}`);
    const identity = parseIdentity(await response.json());
    if (!identity) throw new Error('Unexpected response from /api/identity/me');
    return identity;
};
