import { Link } from '@tanstack/react-router';
import { Plus } from 'lucide-react';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui';
import type { CommunitySummary } from '@/features/communities/domain';
import { cn, initialsOf } from '@/lib/utils';

const RAIL_ITEM =
    'grid size-12 place-items-center rounded-3xl bg-card text-sm font-semibold transition-all hover:rounded-2xl hover:bg-primary hover:text-primary-foreground';
const RAIL_ITEM_ACTIVE = 'rounded-2xl bg-primary text-primary-foreground';

const RailItem = ({ community }: { community: CommunitySummary }) => (
    <Tooltip>
        <TooltipTrigger asChild>
            <Link
                to="/c/$communityId"
                params={{ communityId: community.id }}
                aria-label={community.name}
                className={RAIL_ITEM}
                activeProps={{ className: RAIL_ITEM_ACTIVE }}
            >
                {initialsOf(community.name)}
            </Link>
        </TooltipTrigger>
        <TooltipContent side="right">{community.name}</TooltipContent>
    </Tooltip>
);

/** The servers the user is in, as round buttons, and the way to add one. */
export const ServerRail = ({ communities }: { communities: CommunitySummary[] }) => (
    <>
        {communities.map((community) => (
            <RailItem key={community.id} community={community} />
        ))}
        <Tooltip>
            <TooltipTrigger asChild>
                <Link
                    to="/"
                    aria-label="Add a server"
                    activeOptions={{ exact: true }}
                    className={cn(RAIL_ITEM, 'text-success hover:bg-success')}
                >
                    <Plus className="size-5" />
                </Link>
            </TooltipTrigger>
            <TooltipContent side="right">Create or join a server</TooltipContent>
        </Tooltip>
    </>
);
