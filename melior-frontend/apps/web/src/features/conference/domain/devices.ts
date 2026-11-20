/** The three things a person can change: which microphone, camera and speaker the call uses. */
export type DeviceKind = 'audioinput' | 'videoinput' | 'audiooutput';

/** What the browser reports about one device, reduced to what the call needs. */
export type DeviceInfo = {
    deviceId: string;
    kind: DeviceKind;
    /** Empty until the person has allowed camera or microphone access. */
    label: string;
};

export type DeviceOption = {
    deviceId: string;
    /** Always shown: a device with no label gets a numbered name. */
    label: string;
};

export type DeviceLists = Record<DeviceKind, DeviceOption[]>;

/** The devices the person picked last. A missing entry means "the browser's default". */
export type DevicePreferences = Partial<Record<DeviceKind, string>>;

const KINDS: readonly DeviceKind[] = ['audioinput', 'videoinput', 'audiooutput'];

const FALLBACK_NAME: Record<DeviceKind, string> = {
    audioinput: 'Microphone',
    videoinput: 'Camera',
    audiooutput: 'Speaker',
};

const isKind = (value: unknown): value is DeviceKind => KINDS.some((kind) => kind === value);

/**
 * Sorts the browser's device list into microphones, cameras and speakers, each with a readable name.
 * A device the browser lists twice (a "default" entry is also listed under its own id) is shown once.
 */
export const groupDevices = (devices: readonly DeviceInfo[]): DeviceLists => {
    const lists: DeviceLists = { audioinput: [], videoinput: [], audiooutput: [] };
    for (const device of devices) {
        if (!isKind(device.kind)) continue;
        const list = lists[device.kind];
        if (list.some((option) => option.deviceId === device.deviceId)) continue;
        const label = device.label.trim() || `${FALLBACK_NAME[device.kind]} ${list.length + 1}`;
        list.push({ deviceId: device.deviceId, label });
    }
    return lists;
};

/**
 * The device to show as selected: the one in use if it still exists, otherwise the person's earlier
 * choice if it does, otherwise the first one. Empty when there is no device of this kind.
 */
export const selectedDevice = (options: readonly DeviceOption[], inUse: string | null, preferred?: string): string => {
    for (const wanted of [inUse, preferred]) {
        if (wanted && options.some((option) => option.deviceId === wanted)) return wanted;
    }
    return options[0]?.deviceId ?? '';
};

/** Reads the stored preferences. Anything that is not a map from a device kind to a string is dropped. */
export const parseDevicePreferences = (raw: string | null): DevicePreferences => {
    if (!raw) return {};
    let value: unknown;
    try {
        value = JSON.parse(raw);
    } catch {
        return {};
    }
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return {};
    const preferences: DevicePreferences = {};
    for (const kind of KINDS) {
        const id = (value as Record<string, unknown>)[kind];
        if (typeof id === 'string' && id !== '') preferences[kind] = id;
    }
    return preferences;
};
