import { isObject, isString, parseIceCandidate } from './parse-primitives';
import { PATHS, type Path, type ServerMessage } from './signalling-message';

const isId = (value: unknown): value is string => isString(value) && value.length > 0;

const isEpoch = (value: unknown): value is number =>
    typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;

const isPath = (value: unknown): value is Path => PATHS.some((known) => known === value);

/** Parses the messages of the mesh protocol; null for anything else or anything that does not fit. */
export const parseMeshMessage = (channel: string, payload: unknown): ServerMessage | null => {
    if (!isObject(payload)) return null;
    switch (channel) {
        case 'route':
            return isId(payload.peer) && isPath(payload.path) && isEpoch(payload.epoch)
                ? { channel, payload: { peer: payload.peer, path: payload.path, epoch: payload.epoch } }
                : null;
        case 'link_close':
            return isId(payload.peer) ? { channel, payload: { peer: payload.peer } } : null;
        case 'uplink':
            return typeof payload.active === 'boolean' ? { channel, payload: { active: payload.active } } : null;
        case 'peer_offer':
        case 'peer_answer':
            return isId(payload.from) && isString(payload.sdp)
                ? { channel, payload: { from: payload.from, sdp: payload.sdp } }
                : null;
        case 'peer_ice': {
            const candidate = parseIceCandidate(payload.candidate);
            return isId(payload.from) && candidate ? { channel, payload: { from: payload.from, candidate } } : null;
        }
        case 'e2ee':
            return isId(payload.from) && isString(payload.payload)
                ? { channel, payload: { from: payload.from, payload: payload.payload } }
                : null;
        default:
            return null;
    }
};
