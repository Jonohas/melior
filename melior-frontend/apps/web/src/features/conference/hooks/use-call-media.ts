import { type RefObject, useCallback, useRef, useState } from 'react';
import {
    captureInput,
    captureLocalMedia,
    loadCallPreferences,
    loadDevicePreferences,
    saveCallPreferences,
    saveDevicePreference,
    stopMedia,
} from '@/features/conference/api';
import type { CallPreferences } from '@/features/conference/domain';
import type { ConferenceConnection } from './session-messages';

export type ActiveDevices = { audioinput: string | null; videoinput: string | null };

export type CallMedia = {
    /** The microphone and camera in use, by device id. Empty until the camera is open. */
    activeDevices: ActiveDevices;
    /** Whether your microphone is muted: the others receive silence. Remembered for the next call. */
    microphoneMuted: boolean;
    /** Whether your camera is off: the others receive black frames. Remembered for the next call. */
    cameraOff: boolean;
    setMicrophoneMuted: (muted: boolean) => void;
    setCameraOff: (off: boolean) => void;
    /** Switches to another microphone or camera during the call. Rejects when the device cannot be opened. */
    changeInput: (kind: 'audioinput' | 'videoinput', deviceId: string) => Promise<void>;
    /** Opens the camera and microphone the person chose last, with their mute switches applied. */
    open: () => Promise<MediaStream>;
    /** Makes an opened stream the one in use. */
    adopt: (stream: MediaStream) => void;
    /** Stops the stream in use. */
    release: () => void;
};

/** Turns the microphone and camera tracks on or off as the person chose. Off means silence or black frames. */
const applyCallPreferences = (stream: MediaStream, preferences: CallPreferences) => {
    for (const track of stream.getAudioTracks()) track.enabled = !preferences.microphoneMuted;
    for (const track of stream.getVideoTracks()) track.enabled = !preferences.cameraOff;
};

const devicesOf = (stream: MediaStream): ActiveDevices => ({
    audioinput: stream.getAudioTracks()[0]?.getSettings().deviceId ?? null,
    videoinput: stream.getVideoTracks()[0]?.getSettings().deviceId ?? null,
});

/** The camera and microphone: which devices are in use, switching them, and the mute and camera-off switches. */
export const useCallMedia = (connection: RefObject<ConferenceConnection>): CallMedia => {
    const localRef = useRef<MediaStream | null>(null);
    const [preferences, setPreferences] = useState(loadCallPreferences);
    // Read by the capture and by input changes, which must not wait for a render to apply the switches.
    const preferencesRef = useRef(preferences);
    preferencesRef.current = preferences;
    const [activeDevices, setActiveDevices] = useState<ActiveDevices>({ audioinput: null, videoinput: null });

    const open = useCallback(async () => {
        const stream = await captureLocalMedia(loadDevicePreferences());
        applyCallPreferences(stream, preferencesRef.current);
        return stream;
    }, []);

    const adopt = useCallback((stream: MediaStream) => {
        localRef.current = stream;
        setActiveDevices(devicesOf(stream));
    }, []);

    const release = useCallback(() => {
        if (localRef.current) stopMedia(localRef.current);
        localRef.current = null;
    }, []);

    const changeInput = useCallback(
        async (kind: 'audioinput' | 'videoinput', deviceId: string) => {
            const stream = localRef.current;
            if (!stream) return;
            const track = await captureInput(kind, deviceId);
            const old = kind === 'audioinput' ? stream.getAudioTracks()[0] : stream.getVideoTracks()[0];
            try {
                await connection.current.peer?.replaceInput(track);
            } catch (error) {
                track.stop();
                throw error;
            }
            if (old) {
                stream.removeTrack(old);
                old.stop();
            }
            stream.addTrack(track);
            applyCallPreferences(stream, preferencesRef.current);
            saveDevicePreference(kind, deviceId);
            setActiveDevices(devicesOf(stream));
        },
        [connection],
    );

    const update = useCallback((change: Partial<CallPreferences>) => {
        const next = { ...preferencesRef.current, ...change };
        preferencesRef.current = next;
        setPreferences(next);
        saveCallPreferences(next);
        if (localRef.current) applyCallPreferences(localRef.current, next);
    }, []);
    const setMicrophoneMuted = useCallback((muted: boolean) => update({ microphoneMuted: muted }), [update]);
    const setCameraOff = useCallback((off: boolean) => update({ cameraOff: off }), [update]);

    return {
        activeDevices,
        microphoneMuted: preferences.microphoneMuted,
        cameraOff: preferences.cameraOff,
        setMicrophoneMuted,
        setCameraOff,
        changeInput,
        open,
        adopt,
        release,
    };
};
