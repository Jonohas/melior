export const queryKeys = {
    auth: {
        me: ['auth', 'me'] as const,
    },
    communities: {
        /** The prefix every community query hangs off. */
        all: ['communities'] as const,
        list: ['communities', 'list'] as const,
        detail: (communityId: string) => ['communities', 'detail', communityId] as const,
        members: (communityId: string) => ['communities', 'members', communityId] as const,
        invites: (communityId: string) => ['communities', 'invites', communityId] as const,
    },
    invites: {
        preview: (code: string) => ['invites', 'preview', code] as const,
    },
} as const;
