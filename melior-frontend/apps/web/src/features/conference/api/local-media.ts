import {
    type CallPreferences,
    DEFAULT_CALL_PREFERENCES,
    type DeviceInfo,
    type DeviceKind,
    type DevicePreferences,
    parseCallPreferences,
    parseDevicePreferences,
} from '@/features/conference/domain';

/**
 * Microphone capture. Remote audio is played by the remote <video> elements; the local preview stays muted.
 * Two tabs on one machine will feed back into each other unless you use headphones.
 */
export const ENABLE_AUDIO = true;

const AUDIO_CONSTRAINTS: MediaTrackConstraints = {
    echoCancellation: true,
    noiseSuppression: true,
    autoGainControl: true,
};

const audioConstraints = (deviceId?: string): MediaTrackConstraints =>
    deviceId ? { ...AUDIO_CONSTRAINTS, deviceId: { exact: deviceId } } : AUDIO_CONSTRAINTS;

const open = (preferences: DevicePreferences): Promise<MediaStream> =>
    navigator.mediaDevices.getUserMedia({
        video: preferences.videoinput ? { deviceId: { exact: preferences.videoinput } } : true,
        audio: ENABLE_AUDIO ? audioConstraints(preferences.audioinput) : false,
    });

/**
 * Opens the camera and microphone the person used last. A device that was unplugged since then makes
 * that fail, and the call then uses the browser's defaults instead of not starting.
 */
export const captureLocalMedia = async (preferences: DevicePreferences = {}): Promise<MediaStream> => {
    if (!preferences.videoinput && !preferences.audioinput) return open({});
    try {
        return await open(preferences);
    } catch (error) {
        console.warn('the remembered devices are not available, using the defaults', error);
        return open({});
    }
};

/**
 * Captures one microphone or camera, for switching to it during a call. `exact`: the person picked this
 * device, so any other would be the wrong answer. Rejects when it cannot be opened.
 */
export const captureInput = async (kind: 'audioinput' | 'videoinput', deviceId: string): Promise<MediaStreamTrack> => {
    const stream = await navigator.mediaDevices.getUserMedia(
        kind === 'audioinput' ? { audio: audioConstraints(deviceId) } : { video: { deviceId: { exact: deviceId } } },
    );
    const [track] = kind === 'audioinput' ? stream.getAudioTracks() : stream.getVideoTracks();
    if (!track) throw new Error('The device gave no media.');
    return track;
};

/** Every microphone, camera and speaker the browser lists. Labels are filled in once access was allowed. */
export const listDevices = async (): Promise<DeviceInfo[]> => {
    if (!navigator.mediaDevices?.enumerateDevices) return [];
    const devices = await navigator.mediaDevices.enumerateDevices();
    return devices.map(({ deviceId, kind, label }) => ({ deviceId, kind, label }));
};

/** Calls `onChange` when a device is plugged in or removed. Returns the function that stops listening. */
export const watchDevices = (onChange: () => void): (() => void) => {
    navigator.mediaDevices?.addEventListener('devicechange', onChange);
    return () => navigator.mediaDevices?.removeEventListener('devicechange', onChange);
};

type SinkElement = HTMLMediaElement & { setSinkId?: (deviceId: string) => Promise<void> };

/** Whether this browser can send audio to a chosen speaker. Firefox and Safari cannot, so the choice is left out. */
export const canChooseSpeaker = (): boolean =>
    typeof HTMLMediaElement !== 'undefined' &&
    typeof (HTMLMediaElement.prototype as SinkElement).setSinkId === 'function';

/** Plays an element's sound on a chosen speaker. An empty id means the browser's default. */
export const setSpeaker = async (element: HTMLMediaElement, deviceId: string): Promise<void> => {
    const sink = element as SinkElement;
    if (sink.setSinkId) await sink.setSinkId(deviceId);
};

const PREFERENCES_KEY = 'melior.devices';

export const loadDevicePreferences = (): DevicePreferences => {
    try {
        return parseDevicePreferences(window.localStorage.getItem(PREFERENCES_KEY));
    } catch {
        return {};
    }
};

/** Remembers a choice for the next call. A browser that refuses storage just forgets it. */
export const saveDevicePreference = (kind: DeviceKind, deviceId: string): void => {
    try {
        const next = { ...loadDevicePreferences(), [kind]: deviceId };
        window.localStorage.setItem(PREFERENCES_KEY, JSON.stringify(next));
    } catch {
        // Private browsing or a full quota: the choice still applies to this call.
    }
};

export const stopMedia = (stream: MediaStream): void => {
    for (const track of stream.getTracks()) track.stop();
};

/** Whether this browser can capture a screen at all (phones and some embedded browsers cannot). */
export const canShareScreen = (): boolean =>
    typeof navigator !== 'undefined' && typeof navigator.mediaDevices?.getDisplayMedia === 'function';

/**
 * The sound of a shared screen is played back as it is: no processing meant for a voice, which would cut music
 * and game sound. `systemAudio` lets the picker offer the whole system's sound (Chromium on Windows and ChromeOS);
 * elsewhere only a tab's sound can be shared. Browsers that do not know an option ignore it.
 */
const SCREEN_CAPTURE_OPTIONS = {
    video: true,
    audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false },
    systemAudio: 'include',
} as DisplayMediaStreamOptions;

/**
 * Asks the person to pick a screen, window or tab and captures it, with its sound when they tick the picker's
 * "share audio" box. Rejects with a `NotAllowedError` when they cancel the picker.
 */
export const captureScreen = (): Promise<MediaStream> => navigator.mediaDevices.getDisplayMedia(SCREEN_CAPTURE_OPTIONS);

/** True when the person closed the screen picker or refused to share, which is not an error to report. */
export const isScreenShareCancelled = (error: unknown): boolean =>
    error instanceof DOMException && (error.name === 'NotAllowedError' || error.name === 'AbortError');

const CALL_PREFERENCES_KEY = 'melior.call';

export const loadCallPreferences = (): CallPreferences => {
    try {
        return parseCallPreferences(window.localStorage.getItem(CALL_PREFERENCES_KEY));
    } catch {
        return DEFAULT_CALL_PREFERENCES;
    }
};

/** Remembers whether the microphone and camera are on, for the next call. */
export const saveCallPreferences = (preferences: CallPreferences): void => {
    try {
        window.localStorage.setItem(CALL_PREFERENCES_KEY, JSON.stringify(preferences));
    } catch {
        // Private browsing or a full quota: the choice still applies to this call.
    }
};
