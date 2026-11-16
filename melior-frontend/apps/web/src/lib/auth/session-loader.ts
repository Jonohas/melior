import { redirect } from '@tanstack/react-router';
import { clearLoginCompleted } from './bff-cookies';
import { SessionRejectedError, UnauthorizedError } from './errors';
import { clearLoginAttempts } from './login-attempt';
import { beginLoginRedirect } from './login-gate';

/** Turns the gate's answer into a router redirect, or the error the route component renders. Never returns. */
const failClosed = (appKey: string, locationHref: string, preload: boolean): never => {
    const gate = beginLoginRedirect(appKey, locationHref, { preload });
    if (gate.kind === 'redirect') throw redirect({ reloadDocument: true, href: gate.href });
    throw new SessionRejectedError(gate.reason);
};

type LoadSessionArgs<TAuth> = {
    loadAuth: () => Promise<TAuth>;
    locationHref: string;
    preload?: boolean;
};

/**
 * The `beforeLoad` envelope for the authenticated layout. A 401 (UnauthorizedError) goes through the login
 * gate and carries the navigation target, so the user lands where they were heading. Other errors, such as
 * ForbiddenError, pass through to the route's error component.
 */
export const loadSession = async <TAuth>(
    appKey: string,
    { loadAuth, locationHref, preload = false }: LoadSessionArgs<TAuth>,
): Promise<{ auth: TAuth }> => {
    try {
        const auth = await loadAuth();
        // The session works, so earlier attempts are settled; drop them before a later genuine expiry
        // is mistaken for a failed login.
        clearLoginAttempts(appKey);
        clearLoginCompleted();
        return { auth };
    } catch (error) {
        if (error instanceof UnauthorizedError) failClosed(appKey, locationHref, preload);
        throw error;
    }
};
