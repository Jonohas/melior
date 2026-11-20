import { isChannelId } from './channel-id';
import {
    type ChatPayload,
    CLOSE_REASONS,
    type CloseReason,
    type IceCandidate,
    type IceServer,
    type Participant,
    type ServerMessage,
    TRACK_SOURCES,
    type TrackAssignment,
} from './signalling-message';

type JsonObject = Record<string, unknown>;

const isObject = (value: unknown): value is JsonObject =>
    typeof value === 'object' && value !== null && !Array.isArray(value);

const isString = (value: unknown): value is string => typeof value === 'string';

const optionalString = (value: unknown): value is string | undefined => value === undefined || isString(value);

const nullableString = (value: unknown): value is string | null | undefined =>
    value === undefined || value === null || isString(value);

const nullableNumber = (value: unknown): value is number | null | undefined =>
    value === undefined || value === null || (typeof value === 'number' && Number.isFinite(value));

const isWholeNumber = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value);

const parseChatPayload = (value: unknown): ChatPayload | null => {
    if (!isObject(value) || !isWholeNumber(value.id) || !isWholeNumber(value.sent_at_ms)) return null;
    if (!isString(value.username) || !isString(value.text) || !optionalString(value.client_id)) return null;
    const payload: ChatPayload = {
        id: value.id,
        username: value.username,
        text: value.text,
        sent_at_ms: value.sent_at_ms,
    };
    if (value.client_id !== undefined) payload.client_id = value.client_id;
    return payload;
};

const parseIceServer = (value: unknown): IceServer | null => {
    if (!isObject(value)) return null;
    const { urls, username, credential } = value;
    const urlList = isString(urls) ? [urls] : urls;
    if (!Array.isArray(urlList) || !urlList.every(isString)) return null;
    if (!optionalString(username) || !optionalString(credential)) return null;
    const server: IceServer = { urls: urlList };
    if (username !== undefined) server.username = username;
    if (credential !== undefined) server.credential = credential;
    return server;
};

const parseParticipant = (value: unknown): Participant | null => {
    if (!isObject(value) || !isString(value.session_id) || !isString(value.username)) return null;
    return { session_id: value.session_id, username: value.username };
};

const parseTrackAssignment = (value: unknown): TrackAssignment | null => {
    if (!isObject(value) || !isString(value.mid) || !isString(value.session_id)) return null;
    const source = TRACK_SOURCES.find((known) => known === value.source);
    if (!source) return null;
    return { mid: value.mid, session_id: value.session_id, source };
};

const parseIceCandidate = (value: unknown): IceCandidate | null => {
    if (!isObject(value)) return null;
    const { candidate, sdpMid, sdpMLineIndex, usernameFragment } = value;
    if (!optionalString(candidate) || !nullableString(sdpMid) || !nullableNumber(sdpMLineIndex)) return null;
    if (!nullableString(usernameFragment)) return null;
    return { candidate, sdpMid, sdpMLineIndex, usernameFragment };
};

const parseSessionDescription = <T extends 'offer' | 'answer'>(
    value: unknown,
    type: T,
): { type: T; sdp: string } | null => {
    if (!isObject(value) || value.type !== type || !isString(value.sdp)) return null;
    return { type, sdp: value.sdp };
};

const parseEnvelope = (value: unknown): ServerMessage | null => {
    if (!isObject(value) || !isString(value.channel)) return null;
    const { payload } = value;

    switch (value.channel) {
        case 'session': {
            if (!isObject(payload) || !isString(payload.session_id) || !isChannelId(payload.channel_id)) return null;
            if (!isString(payload.username)) return null;
            const rawParticipants = payload.participants ?? [];
            if (!Array.isArray(rawParticipants)) return null;
            const participants = rawParticipants.map(parseParticipant);
            if (participants.some((p) => p === null)) return null;
            const rawServers = payload.ice_servers ?? [];
            if (!Array.isArray(rawServers)) return null;
            const servers = rawServers.map(parseIceServer);
            if (servers.some((s) => s === null)) return null;
            return {
                channel: 'session',
                payload: {
                    session_id: payload.session_id,
                    channel_id: payload.channel_id,
                    username: payload.username,
                    participants: participants as Participant[],
                    ice_servers: servers as IceServer[],
                },
            };
        }
        case 'sdp_answer': {
            const description = parseSessionDescription(payload, 'answer');
            return description ? { channel: 'sdp_answer', payload: description } : null;
        }
        case 'sdp_offer': {
            const description = parseSessionDescription(payload, 'offer');
            if (!description || !isObject(payload)) return null;
            // A missing list means "no section carries a participant's track".
            const rawTracks = payload.tracks ?? [];
            if (!Array.isArray(rawTracks)) return null;
            const tracks = rawTracks.map(parseTrackAssignment);
            if (tracks.some((t) => t === null)) return null;
            return { channel: 'sdp_offer', payload: { ...description, tracks: tracks as TrackAssignment[] } };
        }
        case 'ice': {
            const candidate = parseIceCandidate(payload);
            return candidate ? { channel: 'ice', payload: candidate } : null;
        }
        case 'closed': {
            const reason = isObject(payload) ? payload.reason : undefined;
            return CLOSE_REASONS.find((known) => known === reason)
                ? { channel: 'closed', payload: { reason: reason as CloseReason } }
                : null;
        }
        case 'peer_joined': {
            const participant = parseParticipant(payload);
            return participant ? { channel: 'peer_joined', payload: participant } : null;
        }
        case 'peer_left':
            return isObject(payload) && isString(payload.session_id)
                ? { channel: 'peer_left', payload: { session_id: payload.session_id } }
                : null;
        case 'chat': {
            const chat = parseChatPayload(payload);
            return chat ? { channel: 'chat', payload: chat } : null;
        }
        case 'chat_error': {
            if (!isObject(payload) || !isString(payload.message) || !optionalString(payload.client_id)) return null;
            const refusal: { message: string; client_id?: string } = { message: payload.message };
            if (payload.client_id !== undefined) refusal.client_id = payload.client_id;
            return { channel: 'chat_error', payload: refusal };
        }
        case 'error':
            return isObject(payload) && isString(payload.message)
                ? { channel: 'error', payload: { message: payload.message } }
                : null;
        default:
            return null;
    }
};

/** Validates a raw WebSocket text frame into a typed server message, or returns null if it is malformed/unknown. */
export const parseServerMessage = (text: string): ServerMessage | null => {
    try {
        return parseEnvelope(JSON.parse(text));
    } catch {
        return null;
    }
};
