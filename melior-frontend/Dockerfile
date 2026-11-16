# Install dependencies. One COPY per workspace member keeps this layer cached until a package.json changes.
FROM oven/bun:alpine AS deps
WORKDIR /app
COPY package.json bun.lock* ./
COPY apps/web/package.json apps/web/package.json
RUN bun install --frozen-lockfile

# Build the SPA (TanStack Start SPA mode emits dist/client with index.html).
FROM oven/bun:alpine AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY --from=deps /app/apps/web/node_modules ./apps/web/node_modules
COPY . .
# `bun run build` would also run biome and tsc, which CI covers; only the bundle is needed here.
RUN cd apps/web && bunx vite build

# Production image: nginx serving the static SPA and reverse-proxying /ws, /api, /login, /signin and /signout to the backend instance(s).
# The unprivileged variant runs as the non-root `nginx` user.
FROM nginxinc/nginx-unprivileged:alpine AS runner
COPY --from=builder /app/apps/web/dist/client /usr/share/nginx/html
COPY docker/nginx.conf.template /etc/nginx/templates/default.conf.template
COPY --chmod=755 docker/entrypoint.d/10-melior-server-url.envsh /docker-entrypoint.d/10-melior-server-url.envsh

# Backend (the Rust server), ws:// or http://. Override at container start. MELIOR_SERVER_URLS, a comma-
# separated list of backends, takes precedence when set; rooms are then pinned to one by a consistent hash.
ENV MELIOR_SERVER_URL=http://melior-server:5000
# Limit envsubst to our own placeholder so nginx's own $vars are preserved.
ENV NGINX_ENVSUBST_FILTER='^(MELIOR_UPSTREAM_SCHEME|MELIOR_UPSTREAM_SERVERS)$'

EXPOSE 3000
