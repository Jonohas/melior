import { createFileRoute, Link, Outlet, useParams } from '@tanstack/react-router';
import { ServerOff } from 'lucide-react';
import { StatePage } from '@/components';
import { Button, Skeleton } from '@/components/ui';
import { ChannelSidebar, MemberList } from '@/features/communities/components';
import { isRequestFailure } from '@/features/communities/domain';
import {
    CommunityFeedProvider,
    MemberListProvider,
    useCommunityQuery,
    useRemovalRedirect,
} from '@/features/communities/hooks';

/** One server: its channel column, the live feed for everything below, the channel, and the member list on the right. */
export const Route = createFileRoute('/_authenticated/c/$communityId')({
    component: CommunityLayout,
});

const unavailableTitle = (error: Error): string => {
    if (isRequestFailure(error, 'forbidden')) return 'You are not a member of this server';
    if (isRequestFailure(error, 'not_found')) return 'This server does not exist';
    return 'The server could not be loaded';
};

const CommunityUnavailable = ({ error }: { error: Error }) => {
    const title = unavailableTitle(error);
    return (
        <StatePage
            alert
            icon={ServerOff}
            title={title}
            actions={
                <Button asChild>
                    <Link to="/">Back to your servers</Link>
                </Button>
            }
        >
            {error.message}
        </StatePage>
    );
};

function CommunityLayout() {
    const { communityId } = Route.useParams();
    const { channelId } = useParams({ strict: false });
    const community = useCommunityQuery(communityId);
    const onRemoved = useRemovalRedirect(communityId);

    if (community.isPending) return <Skeleton className="m-4 flex-1" />;
    if (community.error) return <CommunityUnavailable error={community.error} />;

    return (
        <CommunityFeedProvider communityId={communityId} onRemoved={onRemoved}>
            <MemberListProvider>
                <ChannelSidebar view={community.data} activeChannelId={channelId ?? null} />
                <Outlet />
                <MemberList view={community.data} />
            </MemberListProvider>
        </CommunityFeedProvider>
    );
}
