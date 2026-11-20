import { createFileRoute, redirect } from '@tanstack/react-router';
import { Ticket } from 'lucide-react';
import { TopBar } from '@/components';
import { InviteLanding } from '@/features/communities/components';
import { isInviteCode } from '@/features/communities/domain';

/** `/invite/<code>`: a preview of where the invite leads, and joining. */
export const Route = createFileRoute('/_authenticated/invite/$code')({
    beforeLoad: ({ params }) => {
        if (!isInviteCode(params.code)) throw redirect({ to: '/' });
    },
    component: InvitePage,
});

function InvitePage() {
    const { code } = Route.useParams();
    return (
        <div className="flex min-w-0 flex-1 flex-col">
            <TopBar
                title={
                    <>
                        <Ticket className="size-4" aria-hidden="true" /> Invite
                    </>
                }
            />
            <InviteLanding code={code} />
        </div>
    );
}
