import { UserRound } from 'lucide-react';
import { Button } from '@/components/ui';
import { StatePage } from './state-page';

export const NoAccessPage = () => (
    <StatePage
        standalone
        icon={UserRound}
        title="No access"
        actions={
            <Button asChild variant="outline">
                {/* A plain anchor: sign-out is a full-page navigation through the BFF, not a client route. */}
                <a href="/signout">Sign out</a>
            </Button>
        }
    >
        You are signed in, but this account has no Melior role (member or admin). Ask an administrator.
    </StatePage>
);
