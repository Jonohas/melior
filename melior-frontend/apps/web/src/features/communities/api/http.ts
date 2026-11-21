import { CommunityRequestError } from '@/features/communities/domain';
import { UnauthorizedError } from '@/lib/auth';

/** The plain-text reason the server gave for a refusal, when it is short enough to show. */
const reasonOf = async (response: Response): Promise<string | null> => {
    const text = (await response.text().catch(() => '')).trim();
    return text !== '' && text.length < 200 ? text : null;
};

/**
 * Sends a request and maps a refusal to a typed error. A 401 becomes an UnauthorizedError, which the query
 * client's caches run through the login gate; everything else a CommunityRequestError with the server's reason.
 */
export const send = async (url: string, init?: RequestInit): Promise<Response> => {
    const response = await fetch(url, init);
    if (response.ok) return response;
    if (response.status === 401) throw new UnauthorizedError();
    throw new CommunityRequestError(response.status, await reasonOf(response));
};

/** A JSON request body with the given method. */
export const withJson = (method: string, body: unknown): RequestInit => ({
    method,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
});

/** Sends, then reads and validates the JSON response; a body that does not fit is an error, never trusted. */
export const fetchParsed = async <T>(
    url: string,
    parse: (raw: unknown) => T | null,
    what: string,
    init?: RequestInit,
): Promise<T> => {
    const response = await send(url, init);
    const parsed = parse(await response.json());
    if (parsed === null) throw new Error(`Unexpected ${what} response`);
    return parsed;
};

/** A path segment from an id. */
export const segment = (value: string): string => encodeURIComponent(value);

/** `base` or `base/<id>`. */
export const withId = (base: string, id?: string): string => (id === undefined ? base : `${base}/${segment(id)}`);
