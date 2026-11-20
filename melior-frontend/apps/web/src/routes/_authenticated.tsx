import { createFileRoute, Outlet } from '@tanstack/react-router';
import { AppShell, NoAccessPage, RouterErrorPage } from '@/components';
import { ServerRail } from '@/features/communities/components';
import { useCommunitiesQuery } from '@/features/communities/hooks';
import { ForbiddenError, loadAuthenticatedContext, useAuth } from '@/lib/auth';

/**
 * The authenticated shell, mounted once for every page. Pathless, so no URL carries `_authenticated`.
 *
 * `beforeLoad` is what makes it authenticated: it loads the identity through the BFF and, on a 401,
 * routes through the single login gate with a full-page redirect. No page under here renders before that
 * resolves. A 403 (signed in, no Melior role) renders the no-access state instead of looping to login.
 */
export const Route = createFileRoute('/_authenticated')({
    beforeLoad: ({ context, location, preload }) =>
        loadAuthenticatedContext({
            queryClient: context.queryClient,
            locationHref: location.href,
            preload,
        }),
    component: AuthenticatedLayout,
    errorComponent: ({ error }) =>
        error instanceof ForbiddenError ? <NoAccessPage /> : <RouterErrorPage error={error} />,
});

function AuthenticatedLayout() {
    const { user } = useAuth();
    const communities = useCommunitiesQuery();

    return (
        <AppShell username={user.username} rail={<ServerRail communities={communities.data ?? []} />}>
            <Outlet />
        </AppShell>
    );
}
