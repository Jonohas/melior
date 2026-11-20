/** How the person wants to join a call: with the microphone and the camera on or off. */
export type CallPreferences = {
    microphoneMuted: boolean;
    cameraOff: boolean;
};

export const DEFAULT_CALL_PREFERENCES: CallPreferences = { microphoneMuted: false, cameraOff: false };

/** Reads the stored preferences. A missing or malformed entry means "on", never a surprise "off". */
export const parseCallPreferences = (raw: string | null): CallPreferences => {
    if (!raw) return DEFAULT_CALL_PREFERENCES;
    let value: unknown;
    try {
        value = JSON.parse(raw);
    } catch {
        return DEFAULT_CALL_PREFERENCES;
    }
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return DEFAULT_CALL_PREFERENCES;
    const { microphoneMuted, cameraOff } = value as Record<string, unknown>;
    return { microphoneMuted: microphoneMuted === true, cameraOff: cameraOff === true };
};

/** Whether a local track of this kind sends media (true) or silence and black frames (false). */
export const trackEnabled = (kind: string, preferences: CallPreferences): boolean =>
    kind === 'audio' ? !preferences.microphoneMuted : !preferences.cameraOff;
