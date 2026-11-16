/** Pure helpers for the multi-backend WebSocket router used by the dev and preview servers. */

export type BackendEndpoint = { host: string; port: number; secure: boolean };

/** Host, port and TLS flag of a backend base URL (http, https, ws or wss). */
export const backendEndpoint = (baseUrl: string): BackendEndpoint => {
    const url = new URL(baseUrl);
    const secure = url.protocol === 'https:' || url.protocol === 'wss:';
    return { host: url.hostname, port: url.port ? Number(url.port) : secure ? 443 : 80, secure };
};

/** True for upgrades aimed at the signalling socket: exactly `/ws`, with or without a query. */
export const isSignallingUpgrade = (requestUrl: string | undefined): boolean =>
    new URL(requestUrl ?? '/', 'http://localhost').pathname === '/ws';

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
