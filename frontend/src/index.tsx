import { serve } from "bun";
import index from "./index.html";

const TARGET_API_URL = "http://localhost:5238";

const server = serve({
    routes: {
        // Serve index.html for all unmatched routes.
        "/app": index,
    },

    async fetch(req, server) {
        const url = new URL(req.url);

        // 1. Check for /api prefix first to forward
        if (url.pathname.startsWith("/api")) {
            console.log(`Forwarding request to ${TARGET_API_URL}${url.pathname}`);
            const proxyUrl = new URL(url.pathname + url.search, TARGET_API_URL);

            // Create a new headers object to avoid mutation issues
            const headers = new Headers(req.headers);
            headers.set("host", proxyUrl.host);

            try {
                return await fetch(proxyUrl.toString(), {
                    method: req.method,
                    headers: headers,
                    body: req.body,
                    redirect: "manual",
                });
            } catch (e) {
                return new Response("Bad Gateway", { status: 502 });
            }
        }

        return new Response("Not Found", { status: 404 });
    },

    development: process.env.NODE_ENV !== "production" && {
        // Enable browser hot reloading in development
        hmr: true,

        // Echo console logs from the browser to the server
        console: true,
    },
});

console.log(`🚀 Server running at ${server.url}`);