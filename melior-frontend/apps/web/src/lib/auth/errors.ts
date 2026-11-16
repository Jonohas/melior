/** The BFF answered 401: there is no session, or it expired. */
export class UnauthorizedError extends Error {
    constructor(message = 'Session expired.') {
        super(message);
        this.name = 'UnauthorizedError';
    }
}

/** The BFF answered 403: signed in, but the account holds no Melior role (`member` or `admin`). */
export class ForbiddenError extends Error {
    constructor(message = 'This account has no access to Melior.') {
        super(message);
        this.name = 'ForbiddenError';
    }
}

/** Why the login gate refused to start another redirect. */
export type LoginGateReason =
    /** Already on `/login`; another redirect would bounce in place. */
    | 'already-on-login'
    /** A token exchange just completed and the API still says 401: redirecting repeats it. */
    | 'round-trip-completed'
    /** Too many redirects in the window; the loop is being bounded. */
    | 'budget-exhausted';

/** The gate's answer. A refusal carries its reason so no caller can silently drop it. */
export type LoginGateResult = { kind: 'redirect'; href: string } | { kind: 'refused'; reason: LoginGateReason };

/** Short, user-readable explanation of a refusal. */
export const refusalMessage = (reason: LoginGateReason): string => {
    switch (reason) {
        case 'already-on-login':
            return 'Sign-in is already in progress.';
        case 'round-trip-completed':
            return 'Signing in worked, but the server rejected the session. Signing in again will not help, contact an administrator.';
        case 'budget-exhausted':
            return 'Too many sign-in attempts in a short time. Wait a minute, then try again.';
    }
};

/** Signing in worked, but the server still rejects the session. Thrown instead of redirecting. */
export class SessionRejectedError extends Error {
    readonly reason: LoginGateReason;
    /** Only the budget refusal clears on its own, so only it offers a retry. */
    readonly retryable: boolean;

    constructor(reason: LoginGateReason) {
        super(refusalMessage(reason));
        this.name = 'SessionRejectedError';
        this.reason = reason;
        this.retryable = reason === 'budget-exhausted';
    }
}
