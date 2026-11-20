import type { Path, RoomMediaPath } from '@/features/conference/domain';

/** The words for where media comes from: the room badge and the tile markers read them from here. */
type RoomLabel = { text: string; hint: string };

export const roomPathLabel = (path: Exclude<RoomMediaPath, { kind: 'none' }>): RoomLabel => {
    switch (path.kind) {
        case 'peer_to_peer':
            return { text: 'Peer to peer', hint: 'Media goes straight between browsers.' };
        case 'via_server':
            return { text: 'Via server', hint: 'Media goes through the server.' };
        case 'mixed':
            return {
                text: `Mixed · ${path.direct} of ${path.total} direct`,
                hint: 'The room is switching, or some links go through the server.',
            };
    }
};

export const tilePathLabel = (path: Path, username: string | undefined): RoomLabel => {
    const who = username ?? 'this participant';
    return path === 'direct'
        ? { text: 'P2P', hint: `Media from ${who} comes straight from their browser.` }
        : { text: 'Server', hint: `Media from ${who} comes through the server.` };
};
