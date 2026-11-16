import { createRouter } from '@tanstack/react-router';
import { RouterErrorPage, RouterNotFoundPage } from '@/components';
import { createAppQueryClient } from '@/lib/query-client';
import { routeTree } from './routeTree.gen';

export function getRouter() {
    return createRouter({
        routeTree,
        context: {
            queryClient: createAppQueryClient(),
        },
        scrollRestoration: true,
        defaultPreload: 'intent',
        defaultPreloadStaleTime: 0,
        defaultErrorComponent: RouterErrorPage,
        defaultNotFoundComponent: RouterNotFoundPage,
    });
}

declare module '@tanstack/react-router' {
    interface Register {
        router: ReturnType<typeof getRouter>;
    }
}
