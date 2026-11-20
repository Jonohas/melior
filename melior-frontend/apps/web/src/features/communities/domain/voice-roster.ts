import type { SessionId, VoiceState } from '@/features/conference/domain';
import type { UserId } from './community';

/** Someone in a voice channel of the community, as the community feed reports them. */
export type VoiceEntry = VoiceState & {
    channelId: string;
    sessionId: SessionId;
    userId: UserId;
    username: string;
};

/** Who is in which voice channel: channel id → session id → entry. */
export type VoiceRoster = Record<string, Record<SessionId, VoiceEntry>>;

export const EMPTY_ROSTER: VoiceRoster = {};

const withoutSession = (roster: VoiceRoster, sessionId: SessionId): VoiceRoster =>
    Object.fromEntries(
        Object.entries(roster)
            .map(([channelId, entries]): [string, Record<SessionId, VoiceEntry>] => {
                const { [sessionId]: _gone, ...rest } = entries;
                return [channelId, rest];
            })
            .filter(([, entries]) => Object.keys(entries).length > 0),
    );

/** Adds or replaces an entry; a session is in one channel at a time. */
export const upsertVoice = (roster: VoiceRoster, entry: VoiceEntry): VoiceRoster => {
    const rest = withoutSession(roster, entry.sessionId);
    return { ...rest, [entry.channelId]: { ...rest[entry.channelId], [entry.sessionId]: entry } };
};

export const removeVoice = (roster: VoiceRoster, sessionId: SessionId): VoiceRoster =>
    withoutSession(roster, sessionId);

/** The roster from a feed snapshot. */
export const rosterOf = (entries: readonly VoiceEntry[]): VoiceRoster => entries.reduce(upsertVoice, EMPTY_ROSTER);

/** The people in one voice channel, by name. */
export const occupantsOf = (roster: VoiceRoster, channelId: string): VoiceEntry[] =>
    Object.values(roster[channelId] ?? {}).sort(
        (a, b) => a.username.localeCompare(b.username) || a.sessionId.localeCompare(b.sessionId),
    );
