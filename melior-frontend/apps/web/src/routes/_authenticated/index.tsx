import { createFileRoute } from '@tanstack/react-router';
import { Server } from 'lucide-react';
import { TopBar } from '@/components';
import { CommunityHome } from '@/features/communities/components';
import { REMOVED_REASONS, type RemovedReason } from '@/features/communities/domain';

type HomeSearch = {
    /** Set after the user was removed from a server, to say why. */
    removed?: RemovedReason;
};

export const Route = createFileRoute('/_authenticated/')({
    validateSearch: (raw: Record<string, unknown>): HomeSearch => {
        const removed = REMOVED_REASONS.find((reason) => reason === raw.removed);
        return removed ? { removed } : {};
    },
    component: HomePage,
});

function HomePage() {
    const { removed } = Route.useSearch();
    return (
        <div className="flex min-w-0 flex-1 flex-col">
            <TopBar
                title={
                    <>
                        <Server className="size-4" aria-hidden="true" /> Servers
                    </>
                }
            />
            <CommunityHome removed={removed} />
        </div>
    );
}
