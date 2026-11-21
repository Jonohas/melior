import { createFileRoute } from '@tanstack/react-router';
import { EyeOff } from 'lucide-react';
import { StatePage } from '@/components';
import { TextChannelView, VoiceChannelView } from '@/features/communities/components';
import { findChannel } from '@/features/communities/domain';
import { useCommunityQuery } from '@/features/communities/hooks';
import { parseForceRelay } from '@/features/conference/domain';
import { useAuth } from '@/lib/auth';

type ChannelSearch = {
    relay?: boolean;
};

/** One channel: a text channel's messages, or a voice channel's call. */
export const Route = createFileRoute('/_authenticated/c/$communityId/$channelId')({
    // `?relay=1` (or `true`) forces all media through the TURN relay, to verify that path works.
    validateSearch: (raw: Record<string, unknown>): ChannelSearch =>
        parseForceRelay(raw.relay) ? { relay: true } : {},
    component: ChannelPage,
});

function ChannelPage() {
    const { communityId, channelId } = Route.useParams();
    const { relay } = Route.useSearch();
    const { user } = useAuth();
    const community = useCommunityQuery(communityId);
    const view = community.data;
    const channel = view ? findChannel(view, channelId) : null;

    if (!view) return null;
    if (!channel) {
        return (
            <StatePage icon={EyeOff} title="Channel not found">
                It was deleted, or you can no longer see it.
            </StatePage>
        );
    }
    if (channel.kind === 'text') return <TextChannelView key={channel.id} channel={channel} username={user.username} />;
    return <VoiceChannelView key={channel.id} view={view} channel={channel} relay={relay === true} />;
}
