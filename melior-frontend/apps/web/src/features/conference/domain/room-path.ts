import type { SessionId } from './session-id';
import type { Path } from './signalling-message';

/** How the room's media reaches this browser right now, as far as it can be told from what is shown. */
export type RoomMediaPath =
    | { kind: 'none' }
    | { kind: 'peer_to_peer' }
    | { kind: 'via_server' }
    | { kind: 'mixed'; direct: number; total: number };

/**
 * The room's media path from the path each remote participant is actually shown over (not from the server's
 * routing intent). A participant with no entry is shown through the server. Nobody else in the room: none.
 */
export const roomMediaPath = (remote: readonly SessionId[], shownPaths: Record<SessionId, Path>): RoomMediaPath => {
    const total = remote.length;
    if (total === 0) return { kind: 'none' };
    const direct = remote.filter((peer) => shownPaths[peer] === 'direct').length;
    if (direct === total) return { kind: 'peer_to_peer' };
    if (direct === 0) return { kind: 'via_server' };
    return { kind: 'mixed', direct, total };
};
