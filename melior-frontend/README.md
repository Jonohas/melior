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

Open `http://localhost:3000/`, create a server (or join one with an invite link) and pick a channel. A voice channel
is a call: open it in two tabs to see each other. `?relay=1` on a voice channel URL forces all media through the TURN
relay (diagnostics). Audio is on, so use headphones.

| Route | Page |
| --- | --- |
| `/` | your servers, create a server, join one by invite link or code; `?removed=kicked\|left\|deleted` says why you were sent here |
| `/c/$communityId` | redirects to the first channel you can see (a text channel if there is one) |
| `/c/$communityId/$channelId` | a text channel's messages, or a voice channel's call with its chat |
| `/invite/$code` | invite preview (server name, member count) and the join button |

## Environment variables

| Variable | Default | Meaning |
| --- | --- | --- |
| `PORT` | `3000` | Dev and preview server port |
| `MELIOR_SERVER_URL` | `ws://127.0.0.1:5000` | Backend. The dev server proxies `/ws` (WebSocket), `/api`, `/login`, `/signin` and `/signout` to it |
| `MELIOR_SERVER_URLS` | unset | Comma-separated backend instances; takes precedence over `MELIOR_SERVER_URL`. `/ws` is routed by a stable hash of the `channel_id` query parameter so a room always reaches the same instance; the community feed `/ws/communities/<id>` works on any instance and is hashed by community id only to spread the feeds; the other paths use the first entry. With one entry it behaves like `MELIOR_SERVER_URL`. With several, Vite's proxy cannot route per request, so `ws-router-plugin.ts` takes over both upgrades (dev and preview) and relays them to the chosen backend unchanged, including Host, Origin, Cookie and the backend's error responses |
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
| `/api/*` | `GET /api/identity/me` returns `{ subject, username, email, roles }`; the community endpoints below |
| `/ws` | a voice channel's signalling WebSocket (`?channel_id=`); needs the session cookie |
| `/ws/communities/{id}` | the community feed; needs the session cookie |

Flow: the pathless `_authenticated` layout route (it wraps every page except `/error`) calls
`GET /api/identity/me` in `beforeLoad`.

- 200: the identity goes into the route context (`useAuth()`), and the server rail's account button shows the username and a `/signout` link.
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

## Servers (communities)

In code the concept is a **community** (`features/communities`), because "server" already means a Melior instance;
the UI calls it a **Server**, as Discord does. The layout is Discord's: the server rail on the left (one round button
per server, plus "create or join"), the server's channel column (its menu, categories that fold, text and voice
channels, and under each voice channel who is in it: a green ring while speaking, a camera icon, a `LIVE` badge while
sharing a screen, a crossed-out microphone while muted), and the channel on the right.

Management UI is shown only to those who may use it (the server checks again regardless): the server menu offers
invites (`create_invite`), server settings (overview: rename for `administrator`/owner, delete for the owner; roles with
permission switches, ordering and deletion below the caller's rank; members with role assignment and kick; invites
for `manage_channels`), channel and category creation (`manage_channels`) and leaving (everyone but the owner).
Channel settings: overview (name, topic, category, delete), media for voice channels (mode, user limit and the mode
and limit in effect, with the server's warning), and permission overrides per role or member with allow / inherit /
deny per channel permission and the switch back to the category's permissions. Category settings edit the overrides
that synced channels use.

The **community feed** (`/ws/communities/{id}`) is opened by the `/c/$communityId` layout (`CommunityFeedProvider`) and
drives the voice roster in the channel list, the live messages of text channels, a refetch of the community on
`structure_changed` (and after every reconnect, for what was missed), and the redirect home on `removed`. It reconnects
like the conference (1 s doubling to 15 s with jitter); a socket that closes before it opens re-fetches the community
first, and a 403 or 404 there ends it as `kicked` or `deleted`.

A text channel loads its history with `GET /api/channels/{id}/messages`, posts with `POST` (the answer settles the
pending message; a refusal marks it failed with the server's reason) and receives new messages from the feed.

Every response and feed message is parsed in `features/communities/domain` (`parseCommunityView`, `parseFeedMessage`,
...) and rejected when it does not fit; permission names the client does not know are dropped. The rules the UI needs
(who may edit which role, kick whom, the channel tree order, override editing, media settings checks) are pure
functions there with tests.

| Request | Used for |
| --- | --- |
| `GET/POST /api/communities`, `GET/PATCH/DELETE /api/communities/{id}` | server list, create, view, rename, delete |
| `GET /api/communities/{id}/members`, `DELETE .../members/{user_id}`, `PUT .../members/{user_id}/roles` | members, kick or leave, roles |
| `POST /api/communities/{id}/roles`, `PATCH/DELETE .../roles/{role_id}` | roles; moving swaps two positions |
| `POST /api/communities/{id}/categories`, `PATCH/DELETE .../categories/{id}` | categories and their overrides |
| `POST /api/communities/{id}/channels`, `PATCH/DELETE /api/channels/{id}` | channels, media settings, overrides, sync |
| `GET/POST /api/channels/{id}/messages` | text and call chat history; text channel posts |
| `GET /api/channels/{id}/admission` | the call's reconnect probe: 204, 403, 404, 409 |
| `POST/GET /api/communities/{id}/invites`, `DELETE /api/invites/{code}` | invites |
| `GET/POST /api/invites/{code}` | invite preview and join |

## Voice state

Each participant's `{ muted, camera, screen, speaking }` travels on the room socket. The client sends `voice_state`
whenever its microphone mute, camera switch or screen share changes, when the speaking flag changes, and once per new
connection. Speaking is measured on the local microphone with a Web Audio `AnalyserNode` (RMS level, sampled every
50 ms, threshold 0.04, held for 300 ms after the level drops; muted is never speaking). The server accepts
`screen: true` only once it has the screen track, so the claim is repeated after each `sdp_answer` until the server's
own `voice_state` echo confirms it. The server's `voice_state` and the `voice` object on `session.participants[]` and
`peer_joined` (missing means everything off) feed the call tiles: a green border while speaking and a "Muted" badge.

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
| `removed` | stop for good: "You were removed from this channel", with a way back to the server |
| `room_moved` | reconnect immediately (the room is hosted by another instance now) |

Any other loss reconnects automatically: the socket closing or erroring after it opened, the peer connection
`failed`, or the socket closing before it opened while the session is still valid (a 503 from an instance that is
restarting or does not host the room). The peer connection and socket are torn down, remote videos and the roster are
cleared, and a fresh socket is opened after an exponential backoff (1 s doubling to a 15 s cap, with jitter, restarted
by any connection that reaches `connected`). The status line shows "Reconnecting… (attempt N)". A close before open
also re-checks the session and then `GET /api/channels/{channel_id}/admission`: 404 stops with "This channel does not
exist", 403 with "You were removed from this channel" and 409 with "This channel is full"; anything else retries. The
policy lives in the pure functions of `features/conference/domain/reconnect.ts`.

## Production image

The `Dockerfile` builds the SPA with Vite and serves it from `nginxinc/nginx-unprivileged` (non-root, port 3000).
nginx falls back to `index.html` for client routes and reverse-proxies `/ws` and `/ws/communities/` (with WebSocket
upgrade headers), `/api`, `/login`, `/signin` and `/signout` to the backend. Configure the backend at container start:

```bash
docker build -t melior-frontend .
docker run -p 3000:3000 -e MELIOR_SERVER_URL=http://melior-server:5000 melior-frontend
```

`MELIOR_SERVER_URL` accepts `http(s)://` or `ws(s)://`; the nginx config template is `docker/nginx.conf.template`.

With several server instances set `MELIOR_SERVER_URLS=http://server-1:5000,http://server-2:5000` (all entries with the
same scheme; it takes precedence over `MELIOR_SERVER_URL`). nginx builds two upstreams from the list. `/ws` uses
`hash $arg_channel_id consistent`, so a room is pinned to one instance and everyone in it reaches the same one; adding
or removing an instance moves only a share of the rooms. `/ws/communities/` (any instance serves any community feed),
`/api`, `/login`, `/signin` and `/signout` use round robin
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
      _authenticated.tsx     pathless layout: session gate, server rail, account menu with sign-out
      _authenticated/index.tsx                       home: your servers, create, join by invite
      _authenticated/c/$communityId.tsx              a server: channel column and community feed
      _authenticated/c/$communityId/index.tsx        redirect to the first visible channel
      _authenticated/c/$communityId/$channelId.tsx   text channel or call; `relay` is a validated boolean search param
      _authenticated/invite/$code.tsx                invite preview and join
      error.tsx              where the BFF sends failed sign-ins
    lib/auth/              BFF auth client: login gate, attempt budget, session loader, identity fetch
    lib/query-client.ts    query client; query-keys.ts
    components/            app-wide components (header, no-access, router error and not-found pages)
    styles.css, fonts/
    features/communities/  servers: domain (view, feed and invite parsers, permissions, hierarchy, overrides, channel
                           tree, voice roster), api (REST client, feed socket), hooks (queries, mutations, feed
                           provider, text channel chat, voice call), components (rail, channel tree, settings, views)
    features/chat/         chat history, timeline and composer, shared by text channels and calls
    features/conference/
      domain/              pure TypeScript, no React/DOM: channel id, signalling message types and parser,
                           conference status and state reducer, reconnect policy, voice state and speaking
                           detection, relay flag (vitest, co-located *.test.ts)
      api/                 adapters owning browser objects: signalling WebSocket client, RTCPeerConnection
                           wrapper, camera and microphone capture, microphone level meter
      hooks/               application layer: use-conference orchestrates domain + api and exposes view state
      components/          presentation only: props in, JSX out
```

Dependency direction: `components -> hooks -> api -> domain`. Each subfolder has a barrel `index.ts` with named
re-exports. Code outside a feature, and other layers, import only `@/features/<feature>/<layer>` barrels, never deep
paths. File names are kebab-case.
