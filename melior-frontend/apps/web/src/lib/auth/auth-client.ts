import { clearLoginCompleted } from './bff-cookies';
import type { LoginGateResult } from './errors';
import { refusalMessage } from './errors';
import { clearLoginAttempts } from './login-attempt';
import { beginLoginRedirect, buildLoginHref, redirectToLogin } from './login-gate';
import { loadSession } from './session-loader';

export type AuthClient = {
    buildLoginHref: (target: string) => string;
    beginLoginRedirect: (target?: string) => LoginGateResult;
    redirectToLogin: (target?: string) => LoginGateResult;
    loadSession: <TAuth>(args: {
        loadAuth: () => Promise<TAuth>;
        locationHref: string;
        preload?: boolean;
    }) => Promise<{ auth: TAuth }>;
    /** Runs the gate for a 401 seen outside a route load, reporting a refusal instead of failing silently. */
    handleUnauthorized: (onRefused: (message: string) => void) => void;
    clearLoginAttempts: () => void;
};

/** Binds the sessionStorage scope once so no call site threads it through. */
export const createAuthClient = (appKey: string): AuthClient => ({
    buildLoginHref,
    beginLoginRedirect: (target) => beginLoginRedirect(appKey, target),
    redirectToLogin: (target) => redirectToLogin(appKey, target),
    loadSession: (args) => loadSession(appKey, args),
    handleUnauthorized: (onRefused) => {
        const result = redirectToLogin(appKey);
        if (result.kind === 'refused') onRefused(refusalMessage(result.reason));
    },
    clearLoginAttempts: () => {
        clearLoginAttempts(appKey);
        clearLoginCompleted();
    },
});

export const auth: AuthClient = createAuthClient('melior');
