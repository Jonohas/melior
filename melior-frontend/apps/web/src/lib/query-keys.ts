export const queryKeys = {
    auth: {
        me: ['auth', 'me'] as const,
    },
    rooms: {
        /** The prefix every room query hangs off. */
        all: ['rooms'] as const,
        list: ['rooms', 'list'] as const,
        detail: (channelId: string) => ['rooms', 'detail', channelId] as const,
    },
} as const;
