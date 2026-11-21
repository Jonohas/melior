import {
    createContext,
    type Dispatch,
    type ReactNode,
    useCallback,
    useContext,
    useEffect,
    useReducer,
    useState,
} from 'react';
import {
    type CommunityId,
    IDLE_VOICE_SESSION,
    type RemovedReason,
    type VoiceSession,
    type VoiceSessionAction,
    type VoiceTarget,
    voiceSessionReducer,
} from '@/features/communities/domain';
import { loadDevicePreferences } from '@/features/conference/api';
import { useVoiceCall, type VoiceCall } from './use-voice-call';

export type VoiceSessionValue = {
    /** The channel the call is in, and why the last call was ended for the user. */
    session: VoiceSession;
    /** The call itself; idle while `session.active` is null. */
    call: VoiceCall;
    /** The speaker the call's sound plays on; empty means the browser's default. */
    speakerId: string;
    setSpeakerId: (deviceId: string) => void;
    /** Joins a voice channel, leaving the call in progress first. */
    join: (target: VoiceTarget) => void;
    leave: () => void;
    /** The user is no longer in a community: a call there ends. */
    exitCommunity: (communityId: CommunityId, reason: RemovedReason) => void;
    /** Forgets why the last call ended, once the user has seen it. */
    dismiss: () => void;
};

const VoiceSessionContext = createContext<VoiceSessionValue | null>(null);

/** The tab's one call; available below `VoiceSessionProvider`. */
export const useVoiceSession = (): VoiceSessionValue => {
    const value = useContext(VoiceSessionContext);
    if (!value) throw new Error('useVoiceSession is used outside VoiceSessionProvider');
    return value;
};

/** Runs the call of `session.active`, and hands a stop by the server back to the session. */
const useActiveCall = (session: VoiceSession, dispatch: Dispatch<VoiceSessionAction>): VoiceCall => {
    const channelId = session.active?.channelId ?? null;
    const call = useVoiceCall(channelId, session.active?.relay ?? false);
    const { stopReason } = call.conference;
    useEffect(() => {
        if (stopReason && channelId) dispatch({ type: 'stopped', channelId, reason: stopReason });
    }, [stopReason, channelId, dispatch]);
    return call;
};

/**
 * Holds the call above the routes, so it goes on while the user looks at other channels, other servers or the
 * home page. Mounted once, in the authenticated layout.
 */
export const VoiceSessionProvider = ({ children }: { children: ReactNode }) => {
    const [session, dispatch] = useReducer(voiceSessionReducer, IDLE_VOICE_SESSION);
    const call = useActiveCall(session, dispatch);
    const [speakerId, setSpeakerId] = useState(() => loadDevicePreferences().audiooutput ?? '');
    const join = useCallback((target: VoiceTarget) => dispatch({ type: 'join', target }), []);
    const leave = useCallback(() => dispatch({ type: 'leave' }), []);
    const dismiss = useCallback(() => dispatch({ type: 'dismiss' }), []);
    const exitCommunity = useCallback(
        (communityId: CommunityId, reason: RemovedReason) =>
            dispatch({ type: 'community_exited', communityId, reason }),
        [],
    );
    // The call changes with every media and connection event, so the value is rebuilt on each render anyway.
    const value = { session, call, speakerId, setSpeakerId, join, leave, exitCommunity, dismiss };
    return <VoiceSessionContext.Provider value={value}>{children}</VoiceSessionContext.Provider>;
};
