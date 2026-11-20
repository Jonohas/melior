import { describe, expect, test } from 'vitest';
import { claimedVoiceState, parseVoiceState, SILENT_VOICE, voiceStatesEqual, voiceUpdateToSend } from './voice-state';

const on = { muted: true, camera: true, screen: true, speaking: true };

describe('parseVoiceState', () => {
    test('reads the four booleans and ignores extra keys', () => {
        expect(parseVoiceState({ ...SILENT_VOICE, session_id: 's' })).toEqual(SILENT_VOICE);
        expect(parseVoiceState(on)).toEqual(on);
    });

    test('rejects a missing or mistyped switch', () => {
        expect(parseVoiceState(null)).toBeNull();
        expect(parseVoiceState({ muted: false, camera: false, screen: false })).toBeNull();
        expect(parseVoiceState({ ...SILENT_VOICE, camera: 'yes' })).toBeNull();
        expect(parseVoiceState([])).toBeNull();
    });
});

describe('claimedVoiceState', () => {
    test('a muted microphone does not speak', () => {
        expect(claimedVoiceState(on)).toEqual({ ...on, speaking: false });
        expect(claimedVoiceState({ ...on, muted: false })).toEqual({ ...on, muted: false });
    });
});

describe('voiceUpdateToSend', () => {
    test('sends the first claim and every change, not a repeat', () => {
        expect(voiceUpdateToSend(SILENT_VOICE, null)).toEqual(SILENT_VOICE);
        expect(voiceUpdateToSend(SILENT_VOICE, { ...SILENT_VOICE })).toBeNull();
        expect(voiceUpdateToSend({ ...SILENT_VOICE, camera: true }, SILENT_VOICE)).toEqual({
            ...SILENT_VOICE,
            camera: true,
        });
    });

    test('voiceStatesEqual compares every switch', () => {
        expect(voiceStatesEqual(SILENT_VOICE, { ...SILENT_VOICE, speaking: true })).toBe(false);
        expect(voiceStatesEqual(on, { ...on })).toBe(true);
    });
});
