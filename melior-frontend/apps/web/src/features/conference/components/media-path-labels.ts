import type { EncryptionSummary, Path, RoomMediaPath } from '@/features/conference/domain';

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

const list = (names: string[]): string => names.join(', ');

/** The words of the encryption badge. */
export const encryptionLabel = (summary: Exclude<EncryptionSummary, { kind: 'alone' }>): RoomLabel => {
    switch (summary.kind) {
        case 'secure':
            return {
                text: 'End-to-end encrypted',
                hint: 'Only the people in this call can see and hear it. The login server vouches for every device; the call server forwards encrypted media.',
            };
        case 'waiting':
            return { text: 'Exchanging keys', hint: `Waiting for the keys of ${list(summary.peers)}.` };
        case 'attention': {
            const invalid =
                summary.invalid.length > 0
                    ? `Keys of ${list(summary.invalid)} did not verify; they receive nothing. `
                    : '';
            const unverified =
                summary.unverified.length > 0
                    ? `The login server did not vouch for the device of ${list(summary.unverified)}.`
                    : '';
            const self = summary.self
                ? 'The login server did not vouch for your device, so the others do not send you their media. Reload the page, or log in again. '
                : '';
            return { text: 'Encrypted · unverified devices', hint: `${self}${invalid}${unverified}`.trim() };
        }
    }
};
