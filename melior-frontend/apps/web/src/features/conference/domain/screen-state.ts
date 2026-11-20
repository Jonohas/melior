import { isObject } from './parse-primitives';
import type { SessionId } from './session-id';
import type { Path } from './signalling-message';

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

/**
 * The participants whose screen gets a tile: those whose screen exists on the path their media is shown over.
 * A screen that exists only on the other path waits, so the tile never appears empty.
 */
export const sharingPeers = (
    screens: Record<SessionId, object>,
    directScreens: Record<SessionId, object>,
    shownPaths: Record<SessionId, Path>,
): SessionId[] =>
    [...new Set([...Object.keys(screens), ...Object.keys(directScreens)])].filter(
        (peer) => ((shownPaths[peer] ?? 'forwarded') === 'direct' ? directScreens : screens)[peer] !== undefined,
    );
