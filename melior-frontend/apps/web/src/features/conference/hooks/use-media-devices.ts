import { useCallback, useEffect, useState } from 'react';
import { listDevices, watchDevices } from '@/features/conference/api';
import { type DeviceLists, groupDevices } from '@/features/conference/domain';

const NONE: DeviceLists = { audioinput: [], videoinput: [], audiooutput: [] };

/**
 * The microphones, cameras and speakers the browser lists, kept up to date as devices are plugged in
 * and removed. Names are only filled in after camera access was allowed, so it also reads again when
 * `refreshKey` changes (the settings dialog opening, for example).
 */
export const useMediaDevices = (refreshKey: unknown): DeviceLists => {
    const [lists, setLists] = useState<DeviceLists>(NONE);

    const refresh = useCallback(() => {
        listDevices()
            .then((devices) => setLists(groupDevices(devices)))
            .catch((error) => console.warn('could not list devices', error));
    }, []);

    // biome-ignore lint/correctness/useExhaustiveDependencies: refreshKey only asks for another read
    useEffect(() => {
        refresh();
        return watchDevices(refresh);
    }, [refresh, refreshKey]);

    return lists;
};
