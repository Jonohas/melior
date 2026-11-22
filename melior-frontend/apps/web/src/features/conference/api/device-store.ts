import { createDeviceKey, type DeviceKey } from './e2ee-crypto';

/**
 * What a browser keeps for end-to-end encryption across calls: its own device key. IndexedDB stores the private
 * key as a non-extractable `CryptoKey`, so not even this app can read its bytes. A browser that refuses storage
 * (private mode, a full quota) gets a key for this page load only.
 */
export type DeviceStore = {
    /** This browser's device key, created on first use. */
    deviceKey: () => Promise<DeviceKey>;
};

const DB_NAME = 'melior-e2ee';
const DB_VERSION = 1;
const DEVICE_STORE = 'device';
const DEVICE_ENTRY = 'self';

/** A store in memory: for tests, and for a browser without usable IndexedDB. */
export const createMemoryDeviceStore = (): DeviceStore => {
    let device: Promise<DeviceKey> | null = null;
    return {
        deviceKey: () => {
            device ??= createDeviceKey();
            return device;
        },
    };
};

const request = <T>(req: IDBRequest<T>): Promise<T> =>
    new Promise((resolve, reject) => {
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => reject(req.error);
    });

const openDatabase = (): Promise<IDBDatabase> => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = () => {
        req.result.createObjectStore(DEVICE_STORE);
    };
    return request(req);
};

const isDeviceKey = (value: unknown): value is DeviceKey =>
    typeof value === 'object' &&
    value !== null &&
    (value as DeviceKey).privateKey instanceof CryptoKey &&
    (value as DeviceKey).publicRaw instanceof Uint8Array;

const createIndexedDeviceStore = (db: IDBDatabase): DeviceStore => {
    const store = (name: string, mode: IDBTransactionMode) => db.transaction(name, mode).objectStore(name);
    let device: Promise<DeviceKey> | null = null;

    const loadOrCreate = async (): Promise<DeviceKey> => {
        const stored: unknown = await request(store(DEVICE_STORE, 'readonly').get(DEVICE_ENTRY));
        if (isDeviceKey(stored)) return stored;
        const created = await createDeviceKey();
        await request(store(DEVICE_STORE, 'readwrite').put(created, DEVICE_ENTRY));
        return created;
    };

    return {
        deviceKey: () => {
            device ??= loadOrCreate();
            return device;
        },
    };
};

/** The browser's store, or one in memory when IndexedDB cannot be opened. */
export const openDeviceStore = async (): Promise<DeviceStore> => {
    if (typeof indexedDB === 'undefined') return createMemoryDeviceStore();
    try {
        return createIndexedDeviceStore(await openDatabase());
    } catch (error) {
        console.warn('IndexedDB is not available; this device key lasts for this page only', error);
        return createMemoryDeviceStore();
    }
};

let shared: Promise<DeviceStore> | null = null;

/** The page's one store: every call of this page load uses the same device key. */
export const sharedDeviceStore = (): Promise<DeviceStore> => {
    shared ??= openDeviceStore();
    return shared;
};
