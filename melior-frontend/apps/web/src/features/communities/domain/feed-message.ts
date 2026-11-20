import { type ChatMessage, parseChatMessage } from '@/features/chat/domain';
import { CLOSE_REASONS, type CloseReason, parseVoiceState } from '@/features/conference/domain';
import type { RequestFailure } from './errors';
import { EMPTY_ROSTER, removeVoice, rosterOf, upsertVoice, type VoiceEntry, type VoiceRoster } from './voice-roster';
import { isObject, isString, oneOf, parseList } from './wire';

export const REMOVED_REASONS = ['kicked', 'left', 'deleted'] as const;
/** Why the community feed ended for this user. */
export type RemovedReason = (typeof REMOVED_REASONS)[number];

/** A message of the community feed (`/ws/communities/{id}`), server to client only. */
export type FeedMessage =
    | { channel: 'snapshot'; voice: VoiceEntry[] }
    | { channel: 'voice_joined' | 'voice_updated'; entry: VoiceEntry }
    | { channel: 'voice_left'; channelId: string; sessionId: string }
    | { channel: 'message'; channelId: string; message: ChatMessage }
    | { channel: 'structure_changed' }
    | { channel: 'removed'; reason: RemovedReason }
    /** The login session ended (`session_ended`); the server closes the feed next. */
    | { channel: 'closed'; reason: CloseReason };

export const parseVoiceEntry = (raw: unknown): VoiceEntry | null => {
    const voice = parseVoiceState(raw);
    if (!voice || !isObject(raw) || !isString(raw.channel_id) || !isString(raw.session_id)) return null;
    if (!isString(raw.user_id) || !isString(raw.username)) return null;
    return {
        ...voice,
        channelId: raw.channel_id,
        sessionId: raw.session_id,
        userId: raw.user_id,
        username: raw.username,
    };
};

const parseVoiceLeft = (payload: unknown): FeedMessage | null => {
    if (!isObject(payload) || !isString(payload.channel_id) || !isString(payload.session_id)) return null;
    return { channel: 'voice_left', channelId: payload.channel_id, sessionId: payload.session_id };
};

const parseChannelMessage = (payload: unknown): FeedMessage | null => {
    if (!isObject(payload) || !isString(payload.channel_id)) return null;
    const message = parseChatMessage(payload.message);
    return message ? { channel: 'message', channelId: payload.channel_id, message } : null;
};

const parseVoiceChange = (channel: 'voice_joined' | 'voice_updated', payload: unknown): FeedMessage | null => {
    const entry = parseVoiceEntry(payload);
    return entry ? { channel, entry } : null;
};

const parseEnvelope = (value: unknown): FeedMessage | null => {
    if (!isObject(value) || !isString(value.channel)) return null;
    const { payload } = value;
    switch (value.channel) {
        case 'snapshot': {
            const voice = isObject(payload) ? parseList(payload.voice, parseVoiceEntry) : null;
            return voice ? { channel: 'snapshot', voice } : null;
        }
        case 'voice_joined':
        case 'voice_updated':
            return parseVoiceChange(value.channel, payload);
        case 'voice_left':
            return parseVoiceLeft(payload);
        case 'message':
            return parseChannelMessage(payload);
        case 'structure_changed':
            return { channel: 'structure_changed' };
        case 'removed': {
            const reason = isObject(payload) ? oneOf(REMOVED_REASONS, payload.reason) : null;
            return reason ? { channel: 'removed', reason } : null;
        }
        case 'closed': {
            const reason = isObject(payload) ? oneOf(CLOSE_REASONS, payload.reason) : null;
            return reason ? { channel: 'closed', reason } : null;
        }
        default:
            return null;
    }
};

/** Validates a raw feed frame; null for anything malformed or unknown. */
export const parseFeedMessage = (text: string): FeedMessage | null => {
    try {
        return parseEnvelope(JSON.parse(text));
    } catch {
        return null;
    }
};

/** The voice roster after a feed message; messages about anything else leave it as it is. */
export const applyToRoster = (roster: VoiceRoster, message: FeedMessage): VoiceRoster => {
    switch (message.channel) {
        case 'snapshot':
            return rosterOf(message.voice);
        case 'voice_joined':
        case 'voice_updated':
            return upsertVoice(roster, message.entry);
        case 'voice_left':
            return removeVoice(roster, message.sessionId);
        case 'removed':
            return EMPTY_ROSTER;
        default:
            return roster;
    }
};

export const describeRemoved = (reason: RemovedReason): string => {
    switch (reason) {
        case 'kicked':
            return 'You were removed from the server.';
        case 'left':
            return 'You left the server.';
        case 'deleted':
            return 'The server was deleted.';
    }
};

/** What to do when the feed socket is gone. */
export type FeedCloseDecision = 'stop' | 'check_membership' | 'reconnect';

type FeedEnding = {
    /** The server said `removed`: there is nothing to come back to. */
    removed: boolean;
    /** The server said `closed` (the login session ended): fetching again runs the login gate on a 401. */
    sessionEnded: boolean;
    opened: boolean;
};

/** A socket that never opened may be a refused upgrade (no member any more, or no session), so check first. */
export const decideFeedClose = ({ removed, sessionEnded, opened }: FeedEnding): FeedCloseDecision => {
    if (removed) return 'stop';
    if (sessionEnded || !opened) return 'check_membership';
    return 'reconnect';
};

/**
 * After a feed that never opened, the community was fetched again: 403 means the user is no longer a member,
 * 404 that the community is gone. Anything else (including success) is no reason to stop.
 */
export const removalAfterProbe = (failure: RequestFailure | null): RemovedReason | null => {
    if (failure === 'forbidden') return 'kicked';
    if (failure === 'not_found') return 'deleted';
    return null;
};
