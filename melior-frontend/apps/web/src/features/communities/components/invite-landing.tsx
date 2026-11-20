import { Link, useNavigate } from '@tanstack/react-router';
import { Ticket, TriangleAlert } from 'lucide-react';
import { StatePage } from '@/components';
import { Button, Skeleton } from '@/components/ui';
import { isRequestFailure } from '@/features/communities/domain';
import { useInvitePreviewQuery, useJoinInvite } from '@/features/communities/hooks';
import { FormError } from './form-error';

const BackHome = () => (
    <Button asChild variant="outline">
        <Link to="/">Back to your servers</Link>
    </Button>
);

/** `/invite/<code>`: where the invite leads, and the button to join. */
export const InviteLanding = ({ code }: { code: string }) => {
    const navigate = useNavigate();
    const preview = useInvitePreviewQuery(code);
    const join = useJoinInvite();

    if (preview.isPending) return <Skeleton className="m-6 h-48 flex-1" />;
    if (preview.error) {
        const title = isRequestFailure(preview.error, 'not_found') ? 'Invite not found' : 'Could not load the invite';
        return (
            <StatePage alert icon={TriangleAlert} title={title} actions={<BackHome />}>
                {preview.error.message}
            </StatePage>
        );
    }

    const { community, memberCount, usable } = preview.data;
    const accept = () =>
        join.mutate(code, {
            onSuccess: (communityId) => void navigate({ to: '/c/$communityId', params: { communityId } }),
        });

    return (
        <StatePage
            icon={Ticket}
            title={`You are invited to ${community.name}`}
            actions={
                <>
                    <BackHome />
                    <Button type="button" disabled={!usable || join.isPending} onClick={accept}>
                        Join {community.name}
                    </Button>
                </>
            }
        >
            <p>
                {memberCount} {memberCount === 1 ? 'member' : 'members'}
            </p>
            {!usable && <p>This invite has expired or was used up. Ask for a new one.</p>}
            <FormError message={join.error?.message} />
        </StatePage>
    );
};
