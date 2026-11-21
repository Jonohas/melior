export type ChannelId = string & { readonly __brand: 'ChannelId' };

/** The server's rule for a channel id: generated UUIDs and the names of migrated rooms both fit it. */
export const CHANNEL_ID_PATTERN = /^[A-Za-z0-9_-]{1,64}$/;

export const isChannelId = (value: unknown): value is ChannelId =>
    typeof value === 'string' && CHANNEL_ID_PATTERN.test(value);
