import { createFileRoute, Outlet, useParams } from '@tanstack/react-router';
import { AppShell, NoAccessPage, RouterErrorPage } from '@/components';
import { ServerRail, VoiceCallAudio, VoicePanel } from '@/features/communities/components';
import { useCommunitiesQuery, VoiceSessionProvider } from '@/features/communities/hooks';
import { ForbiddenError, loadAuthenticatedContext, useAuth } from '@/lib/auth';

/**
 * The authenticated shell, mounted once for every page. Pathless, so no URL carries `_authenticated`.
 *
 * `beforeLoad` is what makes it authenticated: it loads the identity through the BFF and, on a 401,
 * routes through the single login gate with a full-page redirect. No page under here renders before that
 * resolves. A 403 (signed in, no Melior role) renders the no-access state instead of looping to login.
 *
 * The voice call lives here, above the routes, so it goes on while the user moves between pages.
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
    // A server's pages show the call panel in their channel column; every other page shows it in the corner.
    const { communityId } = useParams({ strict: false });

    return (
        <VoiceSessionProvider>
            <AppShell
                username={user.username}
                rail={<ServerRail communities={communities.data ?? []} />}
                corner={communityId ? undefined : <VoicePanel className="border-t-0" />}
            >
                <Outlet />
            </AppShell>
            <VoiceCallAudio />
        </VoiceSessionProvider>
    );
}
