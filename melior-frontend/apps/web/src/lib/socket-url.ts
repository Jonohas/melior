/** A WebSocket URL on the page's own origin (`ws:` or `wss:` to match the page), for a path with its query. */
export const sameOriginSocketUrl = (pathAndQuery: string): string => {
    const scheme = window.location.protocol === 'https:' ? 'wss' : 'ws';
    return `${scheme}://${window.location.host}${pathAndQuery}`;
};
