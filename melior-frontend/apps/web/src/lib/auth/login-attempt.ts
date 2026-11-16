/**
 * Per-tab budget of login redirects, the fail-safe half of the loop guard. The precise signal is the
 * marker cookie (see bff-cookies.ts); this bounds the loop when that signal is absent.
 *
 * An attempt is a page load that started a redirect, not a call that asked for one: when a session
 * expires every query in flight gets its 401 at once, and billing per call would spend the whole budget
 * on one expiry. A loop reloads the document each round, so billing per page load still bounds it.
 */

export const MAX_ATTEMPTS = 3;

export const ATTEMPT_WINDOW_MS = 60_000;

type Attempt = { at: number; page: number };

/** sessionStorage is absent during SSR and throws in some privacy modes; both read as "no attempts". */
const storage = (): Storage | null => {
    try {
        return typeof window === 'undefined' ? null : window.sessionStorage;
    } catch {
        return null;
    }
};

const storageKey = (appKey: string) => `${appKey}.login-attempts`;

/** `timeOrigin` is fixed for a document's lifetime and differs on every load. */
const currentPage = (): number => performance.timeOrigin;

const parseAttempt = (entry: unknown): Attempt | null => {
    if (typeof entry !== 'object' || entry === null) return null;
    const { at, page } = entry as Record<string, unknown>;
    if (typeof at !== 'number' || !Number.isFinite(at) || typeof page !== 'number') return null;
    return { at, page };
};

/** Attempts still inside the window. Anything unparseable reads as "none". */
const readAttempts = (appKey: string): Attempt[] => {
    let parsed: unknown;
    try {
        const raw = storage()?.getItem(storageKey(appKey)) ?? null;
        if (raw === null) return [];
        parsed = JSON.parse(raw);
    } catch {
        return [];
    }
    if (!Array.isArray(parsed)) return [];
    const cutoff = Date.now() - ATTEMPT_WINDOW_MS;
    return parsed.map(parseAttempt).filter((attempt): attempt is Attempt => attempt !== null && attempt.at > cutoff);
};

/** Records that this page load is starting a login redirect. Repeat calls from it cost nothing. */
export const recordLoginAttempt = (appKey: string): void => {
    const attempts = readAttempts(appKey);
    const page = currentPage();
    if (attempts.some((attempt) => attempt.page === page)) return;
    try {
        storage()?.setItem(storageKey(appKey), JSON.stringify([...attempts, { at: Date.now(), page }]));
    } catch {
        // A suppressed guard is better than a failed redirect.
    }
};

/** True when {@link MAX_ATTEMPTS} page loads in this tab started a redirect inside {@link ATTEMPT_WINDOW_MS}. */
export const isBudgetExhausted = (appKey: string): boolean => readAttempts(appKey).length >= MAX_ATTEMPTS;

/** Clears the budget. Call once the session is known to work. */
export const clearLoginAttempts = (appKey: string): void => {
    try {
        storage()?.removeItem(storageKey(appKey));
    } catch {
        // Already unreadable, so nothing can suppress a later redirect.
    }
};
