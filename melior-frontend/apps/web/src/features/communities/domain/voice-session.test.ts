import { describe, expect, test } from 'vitest';
import type { ChannelId } from '@/features/conference/domain';
import {
    decideVoiceOpen,
    describeCallStatus,
    IDLE_VOICE_SESSION,
    isInCall,
    type VoiceSession,
    type VoiceTarget,
    voicePanelView,
    voiceSessionReducer,
} from './voice-session';

const channel = (id: string) => id as ChannelId;
const names = { communityName: 'Club', channelName: 'Voice' };
const lounge: VoiceTarget = { communityId: 'c1', channelId: channel('lounge'), relay: false, ...names };
const stage: VoiceTarget = { communityId: 'c1', channelId: channel('stage'), relay: false, ...names };
const elsewhere: VoiceTarget = { communityId: 'c2', channelId: channel('far'), relay: false, ...names };
const inLounge: VoiceSession = { active: lounge, ended: null };
const removedFromLounge: VoiceSession = { active: null, ended: { target: lounge, reason: 'removed' } };

describe('voiceSessionReducer', () => {
    test('joining replaces the call in progress and clears an old notice', () => {
        expect(voiceSessionReducer(IDLE_VOICE_SESSION, { type: 'join', target: lounge })).toEqual(inLounge);
        expect(voiceSessionReducer(inLounge, { type: 'join', target: stage })).toEqual({ active: stage, ended: null });
        expect(voiceSessionReducer(removedFromLounge, { type: 'join', target: stage }).ended).toBeNull();
    });

    test('joining the call you are in changes nothing', () => {
        expect(voiceSessionReducer(inLounge, { type: 'join', target: { ...lounge } })).toBe(inLounge);
    });

    test('joining with the relay forced rejoins', () => {
        const relayed = { ...lounge, relay: true };
        expect(voiceSessionReducer(inLounge, { type: 'join', target: relayed }).active).toEqual(relayed);
    });

    test('leaving ends the call without a notice', () => {
        expect(voiceSessionReducer(inLounge, { type: 'leave' })).toEqual(IDLE_VOICE_SESSION);
    });

    test('a stop by the server ends the call and keeps why', () => {
        const action = { type: 'stopped', channelId: lounge.channelId, reason: 'removed' } as const;
        expect(voiceSessionReducer(inLounge, action)).toEqual(removedFromLounge);
    });

    test('a stop that belongs to another channel is ignored', () => {
        const action = { type: 'stopped', channelId: stage.channelId, reason: 'removed' } as const;
        expect(voiceSessionReducer(inLounge, action)).toBe(inLounge);
    });

    test('being removed from the community ends its call with a notice', () => {
        const kicked = voiceSessionReducer(inLounge, { type: 'community_exited', communityId: 'c1', reason: 'kicked' });
        expect(kicked).toEqual(removedFromLounge);
        const deleted = voiceSessionReducer(inLounge, {
            type: 'community_exited',
            communityId: 'c1',
            reason: 'deleted',
        });
        expect(deleted).toEqual(removedFromLounge);
    });

    test('leaving the community ends its call quietly', () => {
        const left = voiceSessionReducer(inLounge, { type: 'community_exited', communityId: 'c1', reason: 'left' });
        expect(left).toEqual(IDLE_VOICE_SESSION);
    });

    test('leaving another community keeps the call', () => {
        const other = voiceSessionReducer(inLounge, { type: 'community_exited', communityId: 'c2', reason: 'kicked' });
        expect(other).toBe(inLounge);
    });

    test('dismissing clears the notice', () => {
        expect(voiceSessionReducer(removedFromLounge, { type: 'dismiss' })).toEqual(IDLE_VOICE_SESSION);
        expect(voiceSessionReducer(inLounge, { type: 'dismiss' })).toBe(inLounge);
    });
});

describe('decideVoiceOpen', () => {
    test('opens the call you are in', () => {
        expect(decideVoiceOpen(inLounge, lounge)).toBe('show');
        expect(isInCall(inLounge, lounge.channelId)).toBe(true);
        expect(isInCall(inLounge, stage.channelId)).toBe(false);
    });

    test('joins when in no call, switches from another one', () => {
        expect(decideVoiceOpen(IDLE_VOICE_SESSION, lounge)).toBe('join');
        expect(decideVoiceOpen(inLounge, stage)).toBe('switch');
        expect(decideVoiceOpen(inLounge, elsewhere)).toBe('switch');
    });

    test('the relay flag only adds itself', () => {
        const relayed = { ...lounge, relay: true };
        expect(decideVoiceOpen(inLounge, relayed)).toBe('switch');
        expect(decideVoiceOpen({ active: relayed, ended: null }, lounge)).toBe('show');
    });

    test('does not walk back into a call the server ended', () => {
        expect(decideVoiceOpen(removedFromLounge, lounge)).toBe('stopped');
        expect(decideVoiceOpen(removedFromLounge, stage)).toBe('join');
    });
});

describe('describeCallStatus', () => {
    test('names every state with a tone', () => {
        expect(describeCallStatus('connected', 0)).toEqual({ text: 'Voice connected', tone: 'success' });
        expect(describeCallStatus('starting', 0).tone).toBe('pending');
        expect(describeCallStatus('connecting', 0)).toEqual(describeCallStatus('negotiating', 0));
        expect(describeCallStatus('reconnecting', 3)).toEqual({ text: 'Reconnecting… (attempt 3)', tone: 'warning' });
        expect(describeCallStatus('error', 0).tone).toBe('danger');
        expect(describeCallStatus('stopped', 0).tone).toBe('danger');
    });
});

describe('voicePanelView', () => {
    const input = { status: 'connected', reconnectAttempt: 0 } as const;

    test('shows the call while you look at anything else', () => {
        expect(voicePanelView({ ...input, session: inLounge, viewingChannelId: null })).toEqual({
            kind: 'connected',
            target: lounge,
            status: { text: 'Voice connected', tone: 'success' },
        });
        expect(voicePanelView({ ...input, session: inLounge, viewingChannelId: stage.channelId }).kind).toBe(
            'connected',
        );
    });

    test('stays away from the call page itself', () => {
        expect(voicePanelView({ ...input, session: inLounge, viewingChannelId: lounge.channelId })).toEqual({
            kind: 'hidden',
        });
    });

    test('says why the server ended the call, except on the page that already says it', () => {
        expect(voicePanelView({ ...input, session: removedFromLounge, viewingChannelId: null })).toEqual({
            kind: 'ended',
            target: lounge,
            message: 'You were removed from this channel',
        });
        expect(voicePanelView({ ...input, session: removedFromLounge, viewingChannelId: lounge.channelId }).kind).toBe(
            'hidden',
        );
    });

    test('shows nothing without a call', () => {
        expect(voicePanelView({ ...input, session: IDLE_VOICE_SESSION, viewingChannelId: null }).kind).toBe('hidden');
    });
});
