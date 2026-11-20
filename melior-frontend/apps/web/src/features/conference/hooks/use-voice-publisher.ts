import { type RefObject, useCallback, useEffect, useRef } from 'react';
import { claimedVoiceState, type SessionId, type VoiceState, voiceUpdateToSend } from '@/features/conference/domain';
import type { ConferenceConnection } from './session-messages';

type VoicePublisherOptions = {
    connection: RefObject<ConferenceConnection>;
    /** The session of the open connection; a new one starts from what the server records on join. */
    sessionId: SessionId | null;
    /** What this client claims: its switches and whether it is speaking. The server shows a screen once its track arrives. */
    switches: VoiceState;
};

/** Sends `voice_state` whenever mute, camera, screen or speaking changes, and once per new connection. */
export const useVoicePublisher = ({ connection, sessionId, switches }: VoicePublisherOptions): void => {
    const lastSent = useRef<VoiceState | null>(null);
    const claimRef = useRef(claimedVoiceState(switches));
    const { muted, camera, screen, speaking } = switches;
    useEffect(() => {
        claimRef.current = claimedVoiceState({ muted, camera, screen, speaking });
    });

    const publish = useCallback(() => {
        const signalling = connection.current.signalling;
        if (!signalling) return;
        const update = voiceUpdateToSend(claimRef.current, lastSent.current);
        if (!update) return;
        if (signalling.send({ channel: 'voice_state', payload: update })) lastSent.current = update;
    }, [connection]);

    // biome-ignore lint/correctness/useExhaustiveDependencies: a new session has nothing sent yet
    useEffect(() => {
        lastSent.current = null;
    }, [sessionId]);

    // biome-ignore lint/correctness/useExhaustiveDependencies: every switch change is a reason to send
    useEffect(() => {
        if (sessionId !== null) publish();
    }, [sessionId, muted, camera, screen, speaking, publish]);
};
