import { isObject } from './parse-primitives';

/** What a participant's call looks like to the others: the four switches the server relays. */
export type VoiceState = {
    muted: boolean;
    camera: boolean;
    screen: boolean;
    speaking: boolean;
};

/** Everything off: what the server records for a participant that just joined. */
export const SILENT_VOICE: VoiceState = { muted: false, camera: false, screen: false, speaking: false };

const VOICE_KEYS = ['muted', 'camera', 'screen', 'speaking'] as const;

/** Validates the four booleans of a `voice` object (extra keys are ignored). Null if any is missing or mistyped. */
export const parseVoiceState = (value: unknown): VoiceState | null => {
    if (!isObject(value)) return null;
    if (!VOICE_KEYS.every((key) => typeof value[key] === 'boolean')) return null;
    return {
        muted: value.muted as boolean,
        camera: value.camera as boolean,
        screen: value.screen as boolean,
        speaking: value.speaking as boolean,
    };
};

export const voiceStatesEqual = (a: VoiceState, b: VoiceState): boolean => VOICE_KEYS.every((key) => a[key] === b[key]);

/** The state this client claims: a muted microphone is never speaking, as the server enforces too. */
export const claimedVoiceState = (switches: VoiceState): VoiceState => ({
    ...switches,
    speaking: switches.muted ? false : switches.speaking,
});

/** What to send now: the claim, unless it is what was sent last on this connection. */
export const voiceUpdateToSend = (claim: VoiceState, lastSent: VoiceState | null): VoiceState | null =>
    lastSent !== null && voiceStatesEqual(claim, lastSent) ? null : claim;
