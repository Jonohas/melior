import { createFileRoute } from '@tanstack/react-router';
import { TriangleAlert } from 'lucide-react';
import { StatePage } from '@/components';
import { Button } from '@/components/ui';
import { buildLoginHref } from '@/lib/auth';

type ErrorSearch = {
    reason?: string;
    redirectTo?: string;
};

/** Where the BFF sends the browser when the OIDC round trip fails. */
export const Route = createFileRoute('/error')({
    validateSearch: (raw: Record<string, unknown>): ErrorSearch => ({
        reason: typeof raw.reason === 'string' ? raw.reason : undefined,
        redirectTo: typeof raw.redirectTo === 'string' ? raw.redirectTo : undefined,
    }),
    component: SignInErrorPage,
});

function SignInErrorPage() {
    const { reason, redirectTo } = Route.useSearch();

    // A direct assignment rather than the login gate: this button is the user retrying by hand, and the
    // gate exists to stop an automatic redirect looping. After a few failed sign-ins the budget is
    // exhausted, and going through it would leave the only visible action dead.
    return (
        <StatePage
            standalone
            alert
            icon={TriangleAlert}
            title="Sign-in failed"
            actions={
                <Button type="button" onClick={() => window.location.assign(buildLoginHref(redirectTo ?? '/'))}>
                    Try again
                </Button>
            }
        >
            {reason ?? 'Sign-in could not be completed. Check the server logs for the underlying error.'}
        </StatePage>
    );
}
