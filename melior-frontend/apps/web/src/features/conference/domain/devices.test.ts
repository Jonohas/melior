import { describe, expect, test } from 'vitest';
import { type DeviceInfo, groupDevices, parseDevicePreferences, selectedDevice } from './devices';

const device = (kind: DeviceInfo['kind'], deviceId: string, label = ''): DeviceInfo => ({ kind, deviceId, label });

describe('groupDevices', () => {
    test('sorts devices by kind and keeps the browser order', () => {
        const lists = groupDevices([
            device('videoinput', 'c1', 'Webcam'),
            device('audioinput', 'm1', 'Headset'),
            device('audiooutput', 's1', 'Speakers'),
            device('audioinput', 'm2', 'Laptop microphone'),
        ]);
        expect(lists.audioinput.map((o) => o.label)).toEqual(['Headset', 'Laptop microphone']);
        expect(lists.videoinput.map((o) => o.label)).toEqual(['Webcam']);
        expect(lists.audiooutput.map((o) => o.label)).toEqual(['Speakers']);
    });

    test('names a device with no label by its kind and position', () => {
        const lists = groupDevices([device('audioinput', 'a'), device('audioinput', 'b'), device('videoinput', 'c')]);
        expect(lists.audioinput.map((o) => o.label)).toEqual(['Microphone 1', 'Microphone 2']);
        expect(lists.videoinput.map((o) => o.label)).toEqual(['Camera 1']);
    });

    test('shows a device once even when it is listed twice', () => {
        const lists = groupDevices([device('audioinput', 'x', 'Headset'), device('audioinput', 'x', 'Headset')]);
        expect(lists.audioinput).toHaveLength(1);
    });

    test('an empty list gives three empty lists, and an unknown kind is ignored', () => {
        const empty = { audioinput: [], videoinput: [], audiooutput: [] };
        expect(groupDevices([])).toEqual(empty);
        expect(groupDevices([{ kind: 'other' as never, deviceId: 'z', label: 'Z' }])).toEqual(empty);
    });
});

describe('selectedDevice', () => {
    const options = [
        { deviceId: 'a', label: 'A' },
        { deviceId: 'b', label: 'B' },
    ];

    test('prefers the device in use, then the earlier choice, then the first', () => {
        expect(selectedDevice(options, 'b', 'a')).toBe('b');
        expect(selectedDevice(options, null, 'b')).toBe('b');
        expect(selectedDevice(options, null)).toBe('a');
    });

    test('skips a device that was unplugged', () => {
        expect(selectedDevice(options, 'gone', 'b')).toBe('b');
        expect(selectedDevice(options, 'gone', 'also-gone')).toBe('a');
    });

    test('is empty when there is nothing to choose from', () => {
        expect(selectedDevice([], 'a', 'b')).toBe('');
    });
});

describe('parseDevicePreferences', () => {
    test('reads the three kinds', () => {
        const raw = JSON.stringify({ audioinput: 'm', videoinput: 'c', audiooutput: 's' });
        expect(parseDevicePreferences(raw)).toEqual({ audioinput: 'm', videoinput: 'c', audiooutput: 's' });
    });

    test('drops anything that does not fit', () => {
        expect(parseDevicePreferences(null)).toEqual({});
        expect(parseDevicePreferences('not json')).toEqual({});
        expect(parseDevicePreferences('[]')).toEqual({});
        expect(parseDevicePreferences('"x"')).toEqual({});
        expect(parseDevicePreferences(JSON.stringify({ audioinput: 3, videoinput: '', other: 'x' }))).toEqual({});
    });
});
