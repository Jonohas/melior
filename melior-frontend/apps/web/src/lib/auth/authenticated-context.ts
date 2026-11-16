import type { QueryClient } from '@tanstack/react-query';
import { queryKeys } from '@/lib/query-keys';
import { auth } from './auth-client';
import { type CurrentIdentity, getCurrentIdentity } from './current-identity';
import { ForbiddenError, UnauthorizedError } from './errors';

export type AuthContext = {
    user: CurrentIdentity;
};

const fetchIdentity = (queryClient: QueryClient, staleTime?: number) =>
    queryClient.fetchQuery({
        queryKey: queryKeys.auth.me,
        queryFn: getCurrentIdentity,
        retry: false,
        staleTime,
        // Tells the query cache not to run the login gate on this 401: the caller already does, and two
        // gates for one expiry spend two attempts.
        meta: { authBootstrap: true },
    });

/** `beforeLoad` of the `_authenticated` layout: loads the identity through the query client. */
export const loadAuthenticatedContext = ({
    queryClient,
    locationHref,
    preload,
}: {
    queryClient: QueryClient;
    locationHref: string;
    preload?: boolean;
}) =>
    auth.loadSession({
        locationHref,
        preload,
        loadAuth: async (): Promise<AuthContext> => ({ user: await fetchIdentity(queryClient) }),
    });

export type SessionCheck = 'ok' | 'unauthorized' | 'forbidden' | 'error';

/**
 * Re-checks the session now, bypassing the cache. For failures that carry no status, such as a WebSocket
 * that closed before it opened. A 401 runs the login gate (full-page redirect, or a refusal message).
 */
export const recheckSession = async (
    queryClient: QueryClient,
    onRefused: (message: string) => void,
): Promise<SessionCheck> => {
    try {
        await fetchIdentity(queryClient, 0);
        return 'ok';
    } catch (error) {
        if (error instanceof UnauthorizedError) {
            auth.handleUnauthorized(onRefused);
            return 'unauthorized';
        }
        if (error instanceof ForbiddenError) return 'forbidden';
        console.error('session re-check failed', error);
        return 'error';
    }
};
