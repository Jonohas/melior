import { MutationCache, QueryCache, QueryClient } from '@tanstack/react-query';
import { isRedirect } from '@tanstack/react-router';
import { auth, UnauthorizedError } from '@/lib/auth';

declare module '@tanstack/react-query' {
    interface Register {
        queryMeta: {
            authBootstrap?: boolean;
        };
    }
}

export const createAppQueryClient = (): QueryClient =>
    new QueryClient({
        defaultOptions: {
            queries: {
                staleTime: 30 * 1000,
                retry: (failureCount, error) => {
                    if (isRedirect(error) || error instanceof UnauthorizedError) return false;
                    return failureCount < 2;
                },
            },
        },
        // A 401 from a mutation (e.g. saving a room) runs the same login gate as one from a query.
        mutationCache: new MutationCache({
            onError: (error) => {
                if (error instanceof UnauthorizedError) auth.handleUnauthorized((message) => console.error(message));
            },
        }),
        queryCache: new QueryCache({
            onError: (error, query) => {
                // The auth bootstrap query is excluded because its caller already routes the 401 through the
                // login gate; handling it here too would spend two attempts on one expiry.
                if (error instanceof UnauthorizedError && !query.meta?.authBootstrap) {
                    auth.handleUnauthorized((message) => console.error(message));
                }
            },
        }),
    });
