import { clearLoginCompleted, hasLoginCompleted } from './bff-cookies';
import type { LoginGateResult } from './errors';
import { isBudgetExhausted, recordLoginAttempt } from './login-attempt';

export const LOGIN_PATH = '/login';

/**
 * Builds the BFF login URL. `target` is an app-relative path (path, search and hash) the BFF returns the
 * user to after signing in. This is a full-page navigation target: never fetch it.
 */
export const buildLoginHref = (target: string): string => `${LOGIN_PATH}?redirectTo=${encodeURIComponent(target)}`;

/** Where the user should return to after login. */
export const currentLocationHref = (): string =>
    `${window.location.pathname}${window.location.search}${window.location.hash}`;

/** True on `/login` itself, not on a path that merely starts with those characters (`/login-help`). */
const isOnLoginPage = (pathname: string): boolean => pathname === LOGIN_PATH || pathname.startsWith(`${LOGIN_PATH}/`);

/**
 * The one gate every redirect-to-login passes. Refuses when redirecting again would loop and says which
 * condition tripped; callers must surface a refusal. The marker check comes before the budget because it
 * is the precise signal (a token exchange demonstrably succeeded and the server still rejects the
 * session); the budget only bounds the loop when that signal never arrives.
 */
export const beginLoginRedirect = (
    appKey: string,
    target: string = currentLocationHref(),
    { preload = false }: { preload?: boolean } = {},
): LoginGateResult => {
    if (isOnLoginPage(window.location.pathname)) return { kind: 'refused', reason: 'already-on-login' };

    if (hasLoginCompleted()) {
        clearLoginCompleted();
        return { kind: 'refused', reason: 'round-trip-completed' };
    }

    if (isBudgetExhausted(appKey)) return { kind: 'refused', reason: 'budget-exhausted' };

    // The router discards a preload's reloadDocument redirect, so billing it would spend budget for a
    // navigation that never happens.
    if (!preload) recordLoginAttempt(appKey);
    return { kind: 'redirect', href: buildLoginHref(target) };
};

/** Full-page redirect to the BFF login endpoint. Returns the gate result so a refusal can be surfaced. */
export const redirectToLogin = (appKey: string, target?: string): LoginGateResult => {
    const result = beginLoginRedirect(appKey, target);
    if (result.kind === 'redirect') window.location.assign(result.href);
    return result;
};
