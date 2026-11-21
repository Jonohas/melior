/// <reference types="vitest/config" />

import { existsSync, readFileSync } from 'node:fs';
import { isAbsolute, resolve } from 'node:path';
import tailwindcss from '@tailwindcss/vite';
import { tanstackStart } from '@tanstack/react-start/plugin/vite';
import viteReact from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import { parseBackendUrls } from './server-targets';
import { multiBackendWsRouter } from './ws-router-plugin';

const workspaceRoot = resolve(import.meta.dirname, '../..');

// MELIOR_SERVER_URLS (comma separated) lists several backend instances, MELIOR_SERVER_URL is the single
// fallback. ws://host:port is accepted but every target is normalised to http(s), which is what /api and the
// OIDC paths need. With several instances a room is pinned to one by a stable hash of its channel_id.
const backendTargets = parseBackendUrls(process.env.MELIOR_SERVER_URLS, process.env.MELIOR_SERVER_URL);
const backendHttpUrl = backendTargets[0] ?? '';

// changeOrigin stays false so the Host header reaches the backend unchanged: the BFF builds the Keycloak
// redirect_uri from its own FRONTEND_URL, and a rewritten Host would mask a mismatch locally that then
// fails in a deployment.
const httpProxy = {
    target: backendHttpUrl,
    changeOrigin: false,
    secure: false,
};

// Optional HTTPS (camera access from other devices needs a secure context). Paths are resolved from
// the workspace root; both files must exist, otherwise the dev server stays on plain HTTP.
const resolveFromRoot = (path: string) => (isAbsolute(path) ? path : resolve(workspaceRoot, path));
const certPath = resolveFromRoot(process.env.TLS_CERT ?? 'localhost.pem');
const keyPath = resolveFromRoot(process.env.TLS_KEY ?? 'localhost-key.pem');
const useTls = existsSync(certPath) && existsSync(keyPath);
console.log(useTls ? `TLS enabled (${certPath}, ${keyPath})` : 'TLS certs not found, serving plain HTTP');

// Everything the backend (the Rust BFF + SFU) owns. /login, /signin and /signout are full-page
// navigations of the OIDC flow, /api is fetched, /ws is the room's signalling WebSocket and /ws/communities/<id>
// the community feed (the '/ws' key is a prefix, so it covers both).
const backendProxy = {
    // With several backends Vite's proxy cannot choose per request, so multiBackendWsRouter owns /ws and
    // /ws/communities/*: rooms hash by channel id, feeds (which any instance serves) by community id.
    ...(backendTargets.length > 1 ? {} : { '/ws': { ...httpProxy, ws: true } }),
    '/api': httpProxy,
    '/login': httpProxy,
    '/signin': httpProxy,
    '/signout': httpProxy,
};

const port = Number(process.env.PORT ?? 3000);

// Host names the dev and preview servers answer to besides localhost, comma separated. Needed when the
// app is reached through a reverse proxy under another name (for example `tailscale serve`): Vite
// refuses requests whose Host it does not know.
const allowedHosts = (process.env.ALLOWED_HOSTS ?? '')
    .split(',')
    .map((host) => host.trim())
    .filter((host) => host !== '');

const config = defineConfig({
    resolve: { tsconfigPaths: true },
    plugins: [
        ...(backendTargets.length > 1 ? [multiBackendWsRouter(backendTargets)] : []),
        tailwindcss(),
        tanstackStart({
            spa: {
                enabled: true,
                prerender: { outputPath: '/index.html' },
            },
        }),
        viteReact(),
    ],
    server: {
        port,
        strictPort: true,
        // Reachable from other devices on the network.
        host: true,
        https: useTls ? { cert: readFileSync(certPath), key: readFileSync(keyPath) } : undefined,
        allowedHosts,
        proxy: backendProxy,
    },
    preview: {
        port,
        host: true,
        allowedHosts,
        proxy: backendProxy,
    },
    test: {
        testTimeout: 120000,
    },
});

export default config;
