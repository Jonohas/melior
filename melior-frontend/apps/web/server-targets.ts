/** Backend selection for the dev and preview proxies. */

const DEFAULT_URL = 'ws://127.0.0.1:5000';

const toHttp = (url: string): string => url.trim().replace(/\/+$/, '').replace(/^ws/, 'http');

/**
 * The backend base URLs in http(s) form. `MELIOR_SERVER_URLS` (comma separated) wins over the single
 * `MELIOR_SERVER_URL`; with neither set the default backend is used.
 */
export const parseBackendUrls = (urls: string | undefined, single: string | undefined): string[] => {
    const list = (urls ?? '')
        .split(',')
        .map(toHttp)
        .filter((url) => url !== '');
    return list.length > 0 ? list : [toHttp(single && single.trim() !== '' ? single : DEFAULT_URL)];
};

/** FNV-1a, 32 bit: small and stable across runs and platforms. */
export const hashString = (value: string): number => {
    let hash = 0x811c9dc5;
    for (let i = 0; i < value.length; i++) {
        hash ^= value.charCodeAt(i);
        hash = Math.imul(hash, 0x01000193) >>> 0;
    }
    return hash >>> 0;
};

/** The backend for a routing key (a room's channel id): the same key always maps to the same backend. */
export const pickBackend = (targets: readonly string[], key: string | null): string =>
    targets[hashString(key ?? '') % targets.length] ?? '';
