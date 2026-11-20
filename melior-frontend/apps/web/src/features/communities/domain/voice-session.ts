import { type ChannelId, type ConferenceStatus, describeStop, type StopReason } from '@/features/conference/domain';
import type { CommunityId } from './community';
import type { RemovedReason } from './feed-message';

/**
 * The voice channel a call is in, and whether its media is forced through the TURN relay. The names are those at
 * joining, for when the community can no longer be loaded (the user was removed from it).
 */
export type VoiceTarget = {
    communityId: CommunityId;
    channelId: ChannelId;
    relay: boolean;
    communityName: string;
    channelName: string;
};

/** A call the server ended, kept until the user has been told why. */
export type EndedCall = { target: VoiceTarget; reason: StopReason };

/** The one call of this tab: the channel it is in (if any), and the last call that was ended for the user. */
export type VoiceSession = { active: VoiceTarget | null; ended: EndedCall | null };

export const IDLE_VOICE_SESSION: VoiceSession = { active: null, ended: null };

export type VoiceSessionAction =
    | { type: 'join'; target: VoiceTarget }
    | { type: 'leave' }
    | { type: 'stopped'; channelId: ChannelId; reason: StopReason }
    | { type: 'community_exited'; communityId: CommunityId; reason: RemovedReason }
    | { type: 'dismiss' };

const sameTarget = (a: VoiceTarget | null, b: VoiceTarget): boolean =>
    a !== null && a.channelId === b.channelId && a.relay === b.relay;

/** Leaving a community ends its call; only a removal the user did not choose is worth a notice. */
const exitCommunity = (session: VoiceSession, communityId: CommunityId, reason: RemovedReason): VoiceSession => {
    const { active } = session;
    if (active?.communityId !== communityId) return session;
    if (reason === 'left') return IDLE_VOICE_SESSION;
    return { active: null, ended: { target: active, reason: 'removed' } };
};

/** At most one call at a time: joining replaces the call in progress and clears an old notice. */
export const voiceSessionReducer = (session: VoiceSession, action: VoiceSessionAction): VoiceSession => {
    switch (action.type) {
        case 'join':
            if (sameTarget(session.active, action.target)) return session;
            return { active: action.target, ended: null };
        case 'leave':
            return IDLE_VOICE_SESSION;
        case 'stopped':
            if (session.active?.channelId !== action.channelId) return session;
            return { active: null, ended: { target: session.active, reason: action.reason } };
        case 'community_exited':
            return exitCommunity(session, action.communityId, action.reason);
        case 'dismiss':
            return session.ended ? { ...session, ended: null } : session;
    }
};

/**
 * What opening a voice channel's page does: `show` the call you are in, `join` it when you are in none, `switch`
 * from another call, or show why the server ended your call there (`stopped`) instead of walking back into it.
 * The relay flag only ever adds itself: opening the channel without `?relay` keeps a relayed call as it is.
 */
export type VoiceOpenDecision = 'show' | 'join' | 'switch' | 'stopped';

export const decideVoiceOpen = (session: VoiceSession, opened: VoiceTarget): VoiceOpenDecision => {
    const { active, ended } = session;
    if (active?.channelId === opened.channelId && (active.relay || !opened.relay)) return 'show';
    if (ended?.target.channelId === opened.channelId) return 'stopped';
    return active ? 'switch' : 'join';
};

/** The voice view is the call's own page only while the call is in that channel. */
export const isInCall = (session: VoiceSession, channelId: ChannelId): boolean =>
    session.active?.channelId === channelId;

export type CallTone = 'success' | 'pending' | 'warning' | 'danger';

export type CallStatusLabel = { text: string; tone: CallTone };

/** How the connection of the call stands, in the words of the voice panel. */
export const describeCallStatus = (status: ConferenceStatus, reconnectAttempt: number): CallStatusLabel => {
    switch (status) {
        case 'connected':
            return { text: 'Voice connected', tone: 'success' };
        case 'starting':
            return { text: 'Opening camera…', tone: 'pending' };
        case 'connecting':
        case 'negotiating':
            return { text: 'Connecting…', tone: 'pending' };
        case 'reconnecting':
            return { text: `Reconnecting… (attempt ${reconnectAttempt})`, tone: 'warning' };
        case 'stopped':
            return { text: 'Disconnected', tone: 'danger' };
        case 'error':
            return { text: 'Connection failed', tone: 'danger' };
    }
};

export type VoicePanelView =
    | { kind: 'hidden' }
    | { kind: 'connected'; target: VoiceTarget; status: CallStatusLabel }
    | { kind: 'ended'; target: VoiceTarget; message: string };

type PanelInput = {
    session: VoiceSession;
    status: ConferenceStatus;
    reconnectAttempt: number;
    /** The channel on screen, if any. Its own page shows the call (or why it ended), so the panel stays away. */
    viewingChannelId: string | null;
};

/** What the "Voice connected" panel shows: the call in progress, why the last one ended, or nothing. */
export const voicePanelView = ({ session, status, reconnectAttempt, viewingChannelId }: PanelInput): VoicePanelView => {
    const { active, ended } = session;
    if (active) {
        if (active.channelId === viewingChannelId) return { kind: 'hidden' };
        return { kind: 'connected', target: active, status: describeCallStatus(status, reconnectAttempt) };
    }
    if (!ended || ended.target.channelId === viewingChannelId) return { kind: 'hidden' };
    return { kind: 'ended', target: ended.target, message: describeStop(ended.reason) };
};
