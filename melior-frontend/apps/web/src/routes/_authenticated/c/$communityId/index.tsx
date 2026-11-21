import { createFileRoute, redirect } from '@tanstack/react-router';
import { EyeOff } from 'lucide-react';
import { StatePage } from '@/components';
import { firstChannel } from '@/features/communities/domain';
import { communityQueryOptions } from '@/features/communities/hooks';

/** `/c/<id>`: straight on to the first channel the user can see, a text one if there is any. */
export const Route = createFileRoute('/_authenticated/c/$communityId/')({
    loader: async ({ context, params }) => {
        // A failure here is shown by the layout, which loads the same query.
        const view = await context.queryClient
            .ensureQueryData(communityQueryOptions(params.communityId))
            .catch(() => null);
        const channel = view ? firstChannel(view) : null;
        if (channel) {
            throw redirect({
                to: '/c/$communityId/$channelId',
                params: { communityId: params.communityId, channelId: channel.id },
                replace: true,
            });
        }
    },
    component: NoChannelPage,
});

function NoChannelPage() {
    return (
        <StatePage icon={EyeOff} title="No channels here">
            There is no channel in this server you can see.
        </StatePage>
    );
}
