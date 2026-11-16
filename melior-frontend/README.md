# melior-frontend

Browser client for Melior, a video-conferencing prototype backed by the Rust SFU in `melior-server`. A Bun
workspace orchestrated by Turborepo, with one app, `apps/web` (`@melior/web`): TanStack Start (SPA mode) on Vite,
React 19, Tailwind 4.

## Setup

```bash
bun install
```

## Run

All commands run from this directory.

```bash
bun dev                 # Vite dev server on http://localhost:3000 (hot reload)
bun run build           # biome + type check + production build into apps/web/dist/client
bun run check-types     # tsc --noEmit through turbo
bun run biome-check     # read-only lint + format check
bun run biome-fix       # lint + format, autofix
bun run test:unit       # vitest through turbo
```

Use `bun run build` and `bun run test:unit`: a bare `bun build` or `bun test` hits Bun's own bundler and test runner,
not these scripts. From `apps/web`, `bun run test` runs vitest directly, and `bun run preview` serves the built app.

Open `http://localhost:3000/`, type a channel name (default `general`) and join. Rooms live at `/rooms/<channel>`;
channel names are `A-Za-z0-9_-`, 1 to 64 long, anything else redirects to the lobby. `?relay=1` on a room URL forces
all media through the TURN relay (diagnostics). Open the same room in two tabs to see each other. Audio is on, so
use headphones.

## Environment variables

| Variable | Default | Meaning |
| --- | --- | --- |
| `PORT` | `3000` | Dev and preview server port |
| `MELIOR_SERVER_URL` | `ws://127.0.0.1:5000` | Backend. The dev server proxies `/ws` (WebSocket), `/api`, `/login`, `/signin` and `/signout` to it |
| `MELIOR_SERVER_URLS` | unset | Comma-separated backend instances; takes precedence over `MELIOR_SERVER_URL`. `/ws` is routed by a stable hash of the `channel_id` query parameter so a room always reaches the same instance; the other paths use the first entry. With one entry it behaves like `MELIOR_SERVER_URL`. With several, Vite's proxy cannot route per request, so `ws-router-plugin.ts` takes over `/ws` upgrades (dev and preview) and relays them to the chosen backend unchanged, including Host, Origin, Cookie and the backend's error responses |
| `ALLOWED_HOSTS` | unset | Extra host names the dev and preview servers answer to, comma separated. Needed behind a reverse proxy such as `tailscale serve` |
| `TLS_CERT` | `localhost.pem` | Certificate path, relative to this directory unless absolute |
| `TLS_KEY` | `localhost-key.pem` | Private key path, same resolution |

TLS is optional: the dev server uses HTTPS only if both files exist, and logs which mode it started in. Browsers allow
camera access only on `localhost` or over HTTPS, so using the app from another device needs a certificate, for
example `mkcert -key-file localhost-key.pem -cert-file localhost.pem localhost <your-lan-ip>`. The browser always
connects same-origin to `/ws?channel_id=...`.

## Login (Keycloak through the backend BFF)

The browser never talks to Keycloak directly. The Rust backend is the BFF: it runs the OIDC round trip and holds
the session in an HttpOnly cookie, and this app only follows its redirects. Proxied to the backend (Vite in dev and
preview, nginx in the image), all same-origin with the `Host` header passed through unchanged:

| Path | Use |
| --- | --- |
| `/login?redirectTo=<path>` | full-page navigation, 302 to Keycloak. Never fetched |
| `/signin?code=&state=` | OIDC callback; sets the session cookie and a readable `bff_login_completed=1` cookie (30 s), then 302 to `redirectTo`. On failure 302 to the SPA route `/error` |
| `/signout` | full-page navigation; ends the session and returns to `/` |
| `/api/*` | `GET /api/identity/me` returns `{ subject, username, email, roles }` |
| `/ws` | signalling WebSocket; needs the session cookie |

Flow: the pathless `_authenticated` layout route (it wraps the lobby and `/rooms/$channelId`) calls
`GET /api/identity/me` in `beforeLoad`.

- 200: the identity goes into the route context (`useAuth()`), and the header shows the username and a `/signout` link.
- 401: full-page redirect to `/login?redirectTo=<path+search+hash>`.
- 403: the "No access" state with a sign-out link (no redirect loop). This means signed in but no `member` or `admin` role.

Loop guard (`src/lib/auth`): the redirect is refused when already on `/login`, when the `bff_login_completed`
cookie is present (a login just completed and the server still says 401; the cookie is consumed), or after 3
attempts in 60 s (counted per page load, kept in `sessionStorage` under `melior.login-attempts`; link preloads are
not counted). A refusal renders an error page instead of looping. If the signalling socket closes before it opens
(a refused upgrade shows no status in the browser), the room re-checks the session and redirects to login or shows
the no-access state. Participant names come from the `session` and `peer_joined` messages and label each video.

The backend's `FRONTEND_URL` must be the origin the browser actually uses (default `http://localhost:3000`), because
Keycloak redirects back to `<FRONTEND_URL>/signin`. Open the app on a different host or port and the callback
lands elsewhere.

## Rooms and access

The lobby lists "Your rooms" (rooms the user owns or is invited to; admins see all), each with its owner, open or
invite-only, a join link and, for owners, a delete button. In a room the owner (`is_owner`, also true for admins)
gets a "Room access" panel: an "invite only" toggle and an editable list of invited usernames. Every change is saved
immediately with `PUT` and the server's answer replaces the cached room; usernames are lower-cased and checked
(1 to 64 of `a-z0-9._@-`) in the client before sending, and server rejections show inline. Other participants see
whether the room is open or invite-only. Joining an unregistered room over the WebSocket registers it as an open room
owned by the joiner. Code: `src/features/rooms/{domain,api,hooks,components}`.

| Request | Meaning |
| --- | --- |
| `GET /api/rooms` | rooms the user owns or is invited to, `Room[]` |
| `GET /api/rooms/{channel_id}` | one `Room`; 404 when not registered yet, 403 when restricted and not invited |
| `PUT /api/rooms/{channel_id}` | body `{ restricted, members }`; creates the room (caller becomes owner) or, for owner/admin, changes it; 400 for invalid usernames or channel id |
| `DELETE /api/rooms/{channel_id}` | owner or admin; 204 |

`Room` is `{ channel_id, owner, restricted, members, is_owner }`.

## Remote media

Who publishes a remote track is never read from the MediaStream id. The server reuses a media section for another
participant's track after the first one left, and a section's `a=msid` cannot change, so `event.streams[0].id` can name
the previous participant. Every server `sdp_offer` therefore carries
`{ "type": "offer", "sdp", "tracks": [{ "mid", "session_id" }] }`: the complete, authoritative list of media sections
that currently carry a participant's track (camera plus microphone are two entries). Sections not listed carry
nothing; a missing `tracks` is read as an empty list; the list replaces any earlier one. `sdp_answer` carries no tracks.

After `setRemoteDescription(offer)` the peer session looks each `mid` up in `getTransceivers()`, takes the receiver's
track and reconciles `mid -> session_id` with `mid -> track` (pure functions `reconcileTracks` and `diffTracks` in
`features/conference/domain/remote-tracks.ts`). It keeps one MediaStream per participant across renegotiations and
adds or removes tracks on it, removals first, so a reused receiver track leaves the old participant's stream before it
joins the new one. A participant without assigned tracks has no stream and no tile (a roster entry alone shows
nothing). The `<video>` keeps the same `srcObject`, and a `play()` call follows every `addtrack` and `removetrack`.
State keeps `remoteStreams` keyed by session id; videos are labelled from the roster.

## Reconnecting

The conference keeps the camera and microphone and replaces everything else after an unexpected loss. The server
sends `{ "channel": "closed", "payload": { "reason" } }` right before it closes the socket:

| `reason` | Client behaviour |
| --- | --- |
| `session_ended` | re-check `/api/identity/me`: 401 runs the login gate, 403 shows the no-access page, 200 reconnects with backoff |
| `removed` | stop for good: "You were removed from this room", with a link to the lobby |
| `room_moved` | reconnect immediately (the room is hosted by another instance now) |

Any other loss reconnects automatically: the socket closing or erroring after it opened, the peer connection
`failed`, or the socket closing before it opened while the session is still valid (a 503 from an instance that is
restarting or does not host the room). The peer connection and socket are torn down, remote videos and the roster are
cleared, and a fresh socket is opened after an exponential backoff (1 s doubling to a 15 s cap, with jitter, restarted
by any connection that reaches `connected`). The status line shows "Reconnecting… (attempt N)". A close before open
also re-checks the session and then `GET /api/rooms/{channel_id}`: 403 there shows "You are not invited to this room"
and does not retry. The policy lives in the pure functions of `features/conference/domain/reconnect.ts`.

## Production image

The `Dockerfile` builds the SPA with Vite and serves it from `nginxinc/nginx-unprivileged` (non-root, port 3000).
nginx falls back to `index.html` for client routes and reverse-proxies `/ws` (with WebSocket upgrade headers),
`/api`, `/login`, `/signin` and `/signout` to the backend. Configure the backend at container start:

```bash
docker build -t melior-frontend .
docker run -p 3000:3000 -e MELIOR_SERVER_URL=http://melior-server:5000 melior-frontend
```

`MELIOR_SERVER_URL` accepts `http(s)://` or `ws(s)://`; the nginx config template is `docker/nginx.conf.template`.

With several server instances set `MELIOR_SERVER_URLS=http://server-1:5000,http://server-2:5000` (all entries with the
same scheme; it takes precedence over `MELIOR_SERVER_URL`). nginx builds two upstreams from the list. `/ws` uses
`hash $arg_channel_id consistent`, so a room is pinned to one instance and everyone in it reaches the same one; adding
or removing an instance moves only a share of the rooms. `/api`, `/login`, `/signin` and `/signout` use round robin
with passive failover (`proxy_next_upstream error timeout http_502 http_503`). `/ws` has no failover on purpose: the
client reconnects instead and the hash picks the instance again. Hostnames are resolved when nginx starts.

## Project structure

```
package.json, turbo.json   workspace root: bun workspaces, dependency catalogs, turbo tasks
biome.json, tsconfig.base.json
docker/                    nginx config template and entrypoint snippet for the production image
apps/web/
  vite.config.ts           Vite + TanStack Start (SPA), dev proxy, optional TLS, vitest
  biome.json               extends the root config; adds import restrictions
  public/                  static files
  src/
    router.tsx             router factory
    router-context.ts      router context type (query client, auth)
    routes/                file-based routes (routeTree.gen.ts is generated and committed)
      __root.tsx             document shell, QueryClientProvider
      _authenticated.tsx     pathless layout: session gate, header with username and sign-out
      _authenticated/index.tsx             lobby: channel form
      _authenticated/rooms/$channelId.tsx  conference page; `relay` is a validated boolean search param
      error.tsx              where the BFF sends failed sign-ins
    lib/auth/              BFF auth client: login gate, attempt budget, session loader, identity fetch
    lib/query-client.ts    query client; query-keys.ts
    components/            app-wide components (header, no-access, router error and not-found pages)
    styles.css, fonts/
    features/rooms/        room list and room access: domain (Room parser, usernames), api, hooks, components
    features/conference/
      domain/              pure TypeScript, no React/DOM: channel id, signalling message types and parser,
                           conference status and state reducer, reconnect policy, relay flag (vitest, co-located *.test.ts)
      api/                 adapters owning browser objects: signalling WebSocket client, RTCPeerConnection
                           wrapper, camera and microphone capture
      hooks/               application layer: use-conference orchestrates domain + api and exposes view state
      components/          presentation only: props in, JSX out
```

Dependency direction: `components -> hooks -> api -> domain`. Each subfolder has a barrel `index.ts` with named
re-exports. Code outside a feature, and other layers, import only `@/features/<feature>/<layer>` barrels, never deep
paths. File names are kebab-case.
