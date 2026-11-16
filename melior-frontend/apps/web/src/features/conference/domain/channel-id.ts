export type ChannelId = string & { readonly __brand: 'ChannelId' };

export const DEFAULT_CHANNEL_ID = 'general' as ChannelId;

export const CHANNEL_ID_PATTERN = /^[A-Za-z0-9_-]{1,64}$/;

export const isChannelId = (value: unknown): value is ChannelId =>
    typeof value === 'string' && CHANNEL_ID_PATTERN.test(value);

/** Returns the value if it is a valid channel id, otherwise the default channel. */
export const parseChannelId = (value: string | null | undefined): ChannelId =>
    isChannelId(value) ? value : DEFAULT_CHANNEL_ID;

/** Lobby input: trims whitespace, an empty entry means the default channel, anything invalid is null. */
export const normalizeChannelInput = (raw: string): ChannelId | null => {
    const trimmed = raw.trim();
    if (trimmed === '') return DEFAULT_CHANNEL_ID;
    return isChannelId(trimmed) ? trimmed : null;
};
