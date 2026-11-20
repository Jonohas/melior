import { isObject } from './parse-primitives';

/** What the sharer tells a peer on a direct link's data channel when it starts or stops sharing. */
export const screenStateMessage = (sharing: boolean): string => JSON.stringify({ screen: sharing });

/**
 * Reads a data-channel message from a peer: whether it shares its screen. Anything that is not exactly a
 * JSON object with a boolean `screen` is hostile or foreign and yields null (ignored).
 */
export const parseScreenState = (data: unknown): boolean | null => {
    if (typeof data !== 'string') return null;
    let value: unknown;
    try {
        value = JSON.parse(data);
    } catch {
        return null;
    }
    return isObject(value) && typeof value.screen === 'boolean' ? value.screen : null;
};
