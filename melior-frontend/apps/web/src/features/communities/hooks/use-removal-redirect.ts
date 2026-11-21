import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useCallback } from 'react';
import type { CommunityId, RemovedReason } from '@/features/communities/domain';
import { queryKeys } from '@/lib/query-keys';
import { useVoiceSession } from './voice-session-context';

/**
 * What happens when the user is no longer in a community: a call in one of its channels ends, its cached view is
 * dropped, the server list refetched, and home says why.
 */
export const useRemovalRedirect = (communityId: CommunityId): ((reason: RemovedReason) => void) => {
    const queryClient = useQueryClient();
    const navigate = useNavigate();
    const { exitCommunity } = useVoiceSession();
    return useCallback(
        (reason: RemovedReason) => {
            exitCommunity(communityId, reason);
            queryClient.removeQueries({ queryKey: queryKeys.communities.detail(communityId) });
            void queryClient.invalidateQueries({ queryKey: queryKeys.communities.list });
            void navigate({ to: '/', search: { removed: reason } });
        },
        [communityId, queryClient, navigate, exitCommunity],
    );
};
