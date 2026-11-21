/** Pure helpers for the multi-backend WebSocket router used by the dev and preview servers. */

export type BackendEndpoint = { host: string; port: number; secure: boolean };

/** Host, port and TLS flag of a backend base URL (http, https, ws or wss). */
export const backendEndpoint = (baseUrl: string): BackendEndpoint => {
    const url = new URL(baseUrl);
    const secure = url.protocol === 'https:' || url.protocol === 'wss:';
    return { host: url.hostname, port: url.port ? Number(url.port) : secure ? 443 : 80, secure };
};

const COMMUNITY_FEED_PATH = /^\/ws\/communities\/([^/]+)$/;

/**
 * What a backend WebSocket upgrade is routed by, or null when the upgrade is not for the backend (Vite's HMR
 * socket, lookalike paths). The room socket (`/ws?channel_id=`) hashes by channel, so everyone in a room reaches
 * the instance that hosts it. The community feed (`/ws/communities/<id>`) works on any instance; it hashes by
 * community only to spread the feeds out.
 */
export const upgradeRoutingKey = (requestUrl: string | undefined): string | null => {
    const url = new URL(requestUrl ?? '/', 'http://localhost');
    if (url.pathname === '/ws') return url.searchParams.get('channel_id') ?? '';
    const feed = COMMUNITY_FEED_PATH.exec(url.pathname);
    return feed ? decodeURIComponent(feed[1] ?? '') : null;
};

/**
 * The upgrade request exactly as the browser sent it: request line and raw headers, in order and with their
 * original case. Host, Origin and Cookie therefore reach the backend untouched.
 */
export const serializeUpgradeRequest = (
    method: string,
    url: string,
    httpVersion: string,
    rawHeaders: readonly string[],
): string => {
    let head = `${method} ${url} HTTP/${httpVersion}\r\n`;
    for (let i = 0; i + 1 < rawHeaders.length; i += 2) head += `${rawHeaders[i]}: ${rawHeaders[i + 1]}\r\n`;
    return `${head}\r\n`;
};
