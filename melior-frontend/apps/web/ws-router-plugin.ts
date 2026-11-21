import type { IncomingMessage } from 'node:http';
import net from 'node:net';
import type { Duplex } from 'node:stream';
import tls from 'node:tls';
import type { Plugin } from 'vite';
import { pickBackend } from './server-targets';
import { backendEndpoint, serializeUpgradeRequest, upgradeRoutingKey } from './ws-router';

/**
 * Routes backend WebSocket upgrades to one of several backends by a stable hash: the room socket (`/ws`) by its
 * `channel_id`, the community feed (`/ws/communities/<id>`) by its community id.
 *
 * Vite's own `server.proxy` has no per-request routing, so with several backends this plugin takes over
 * `/ws` and `/ws/communities/*` (the proxy entry is left out of the config then). It only touches those upgrades:
 * Vite's HMR socket and everything else are left to the other `upgrade` listeners. The request is forwarded byte for
 * byte, and the backend's answer (101, or an error such as 401/403/503) is relayed verbatim.
 */
export const multiBackendWsRouter = (targets: readonly string[]): Plugin => {
    const handleUpgrade = (req: IncomingMessage, socket: Duplex, head: Buffer) => {
        const key = upgradeRoutingKey(req.url);
        if (key === null) return;

        const endpoint = backendEndpoint(pickBackend(targets, key));
        const upstream = endpoint.secure
            ? tls.connect({ host: endpoint.host, port: endpoint.port, servername: endpoint.host })
            : net.connect({ host: endpoint.host, port: endpoint.port });

        const closeBoth = () => {
            socket.destroy();
            upstream.destroy();
        };
        socket.on('error', closeBoth);
        upstream.on('error', () => {
            if (socket.writable)
                socket.end('HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\nContent-Length: 0\r\n\r\n');
            closeBoth();
        });
        socket.on('close', closeBoth);
        upstream.on('close', () => socket.end());

        upstream.on(endpoint.secure ? 'secureConnect' : 'connect', () => {
            upstream.write(
                serializeUpgradeRequest(req.method ?? 'GET', req.url ?? '/', req.httpVersion, req.rawHeaders),
            );
            if (head.length > 0) upstream.write(head);
            socket.pipe(upstream);
            upstream.pipe(socket);
        });
    };

    const attach = (httpServer: { on: (event: 'upgrade', listener: typeof handleUpgrade) => unknown } | null) => {
        httpServer?.on('upgrade', handleUpgrade);
    };

    return {
        name: 'melior-multi-backend-ws-router',
        configureServer: (server) => attach(server.httpServer),
        configurePreviewServer: (server) => attach(server.httpServer),
    };
};
