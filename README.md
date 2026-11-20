# Melior

Melior is a video-conferencing system. Signed-in users join rooms in the
browser; a selective forwarding unit (SFU) written in Rust receives each
participant's audio and video and forwards it to the others in the same room.

## Architecture

- `melior-server/` - the SFU (binary `melior-sfu`), built on axum and webrtc-rs.
  It also acts as the backend-for-frontend (BFF) for login: it runs the OpenID
  Connect flow against Keycloak and keeps sessions in Redis, so tokens never
  reach the browser. Room ownership, invitations and chat history are stored in
  Postgres.
  Media flows over WebRTC on arbitrary UDP ports.
- `melior-frontend/` - a Bun workspace run by Turborepo with one app,
  `apps/web`: TanStack Start (SPA mode) on Vite, React 19, Tailwind 4. In
  development Vite proxies the backend paths; in production nginx does.
- `docker/` - `docker-compose.yaml` with Postgres, Redis, Keycloak, coturn
  (TURN/STUN) and Jaeger, and optionally the application services.

[`docs/architecture.md`](docs/architecture.md) shows all of this in diagrams:
the system, the media paths, the layers, the login and join sequences, room
access, and how several instances share the load.

Both applications follow domain-driven design: the rules live in a pure domain
layer, and everything technical depends on it, never the reverse.

### Server layers (`melior-server/crates`)

The server is a Cargo workspace with one crate per layer, so the dependency
direction is checked by the compiler and by `cargo xtask layers`, not by
convention.

```
melior-domain          L0  conference/  Room aggregate, participants, tracks, negotiation rules,
                                        RoomAccess (owner, open or invite-only, invitations)
                           identity/    User and roles, Username, session id, post-login return path
melior-application     L1  conference/  ConferenceService (join, leave, offer, answer, ICE, track events,
                                        hosting); ports MediaEngine, MediaSession, MediaEvents,
                                        SignalSink, IceServerProvider, RoomLeases
                           auth/        AuthService (login round trip, session checks with token
                                        refresh, logout); ports IdentityProvider, SessionStore
                           rooms/       RoomAccessService (admit, configure, list, delete); port RoomRepository
                           chat/        ChatService (send to the room, history); port ChatRepository
melior-infrastructure  L2  webrtc/      webrtc-rs media engine: peer connections, RTP forwarding with
                                        reusable outlets, keyframe requests
                           oidc/        OpenID Connect identity provider (code flow with PKCE, verification)
                           redis/       session store, token-refresh claims, room leases
                           postgres/    room registry and chat history on sqlx, with embedded migrations
                           turn.rs      per-user, expiring TURN credentials
                           telemetry.rs logging and OpenTelemetry trace export
                           config.rs    environment configuration
melior-interfaces      L2  http/        /login, /signin, /signout, /api/identity/me, /api/rooms, /api/rooms/{id}/messages, /api/health
                           ws/          the signalling WebSocket: wire-format DTOs and connection handler
melior-sfu             L3  main.rs      composition root; tests/ holds the integration tests
xtask/                     cargo xtask layers
```

A crate depends only on crates of a lower layer; the two adapter crates cannot
depend on each other. The domain is pure and synchronous: no WebRTC, HTTP,
Redis, SQL, logging or async code. The application names no concrete
technology. Each room is a consistency boundary: every change to a room, and
the media operations that follow from it, runs under that room's lock.

The workspace also pins its toolchain (`rust-toolchain.toml`), forbids `unsafe`,
denies `unwrap`/`expect`/`panic!` outside tests, and keeps clippy (`all` plus
`pedantic`) and rustdoc at zero warnings. `AGENTS.md` has the rules and the
commands; `.github/workflows/checks.yml` runs them.

### Frontend layers (`melior-frontend/apps/web/src`)

`routes/` (file-based, with an `_authenticated` layout that gates everything),
`lib/auth` (login gate and session loading), and two features,
`features/conference` and `features/rooms`, each split into
`{domain,api,hooks,components}`: pure TypeScript domain, browser adapters, the
application hooks, and presentation. See `melior-frontend/README.md`.

## Prerequisites

- Rust (the version in `melior-server/rust-toolchain.toml`; rustup installs it)
- Bun
- Docker with Compose
- A browser. Camera and microphone access requires HTTPS or `localhost`.

## Local development

1. Create the secrets and start the infrastructure (Postgres, Redis, Keycloak,
   coturn, Jaeger):

   ```sh
   cd docker
   ./init-env.sh          # once: writes docker/.env with random secrets
   docker compose up -d
   ```

   Keycloak takes about half a minute on first start. It imports the `melior`
   realm and four test users whose passwords equal their usernames: `alice`
   (admin, member), `bob` and `carol` (member), and `guest` (no role, so no
   access). The admin console is at `http://localhost:8300`.

2. Start the server with the secrets from `docker/.env`:

   ```sh
   cd melior-server
   set -a; . ../docker/.env; set +a
   DATABASE_URL="postgres://$POSTGRES_USER:$POSTGRES_PASSWORD@127.0.0.1:5432/$POSTGRES_DB" \
   MELIOR_TURN_URLS='turn:127.0.0.1:3478?transport=udp' MELIOR_TURN_SECRET="$TURN_SECRET" \
   OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4317 \
   cargo run
   ```

   Only `DATABASE_URL` is required; TURN and trace export are optional.

   For anything beyond a quick look, run `cargo run --release`. The media
   path (SRTP, RTP forwarding) is several times slower in a debug build, which
   shows up as late video and audio as soon as the machine is busy.

3. Start the frontend:

   ```sh
   cd melior-frontend
   bun install
   bun dev
   ```

4. Open `http://localhost:3000`, sign in, pick a room. To see two participants
   on one machine, sign in as a second user in a private window. Audio is on:
   use headphones, or the tabs will feed back into each other.

Use `localhost`, not `127.0.0.1`: the origin must be the server's
`FRONTEND_URL`, because Keycloak redirects back to `<FRONTEND_URL>/signin` and
the server only accepts WebSockets and changes from that origin.

## Login

The flow follows the backend-for-frontend (BFF) pattern. All paths are
same-origin for the browser; the frontend proxies them to the server.

| Path | What it does |
| --- | --- |
| `GET /login?redirectTo=<path>` | Starts the code flow with PKCE (S256) and a nonce; 302 to Keycloak. The verifier and nonce wait in a 10-minute HttpOnly cookie (`bff_pkce`). |
| `GET /signin?code=&state=` | Keycloak's callback. Redeems the code, verifies the tokens' signatures, issuer, expiry, audience and nonce, stores the session in Redis and sets the session cookie; 302 to the requested path. Failures go to `/error?reason=...`. |
| `GET /signout` | Deletes the session and 302s through Keycloak's logout back to `/`. |
| `GET /api/identity/me` | `{ subject, username, email, roles }`. 401 without a session, 403 for a user without a Melior role, 503 if the session cannot be verified right now. |
| `GET /api/health` | Liveness; no session needed. |

- The session cookie (`melior_session_id`) is opaque, HttpOnly, `SameSite=Lax`,
  host-only, and `Secure` when `FRONTEND_URL` is https.
- A session lives `SESSION_TTL_SECONDS` in Redis. Reading it does not extend
  it; a successful token refresh does. Tokens are refreshed shortly before the
  access token expires. A refresh Keycloak rejects ends the session; a refresh
  that cannot be attempted (Keycloak unreachable) leaves the session in place.
- With several server instances, a claim in Redis makes sure only one of them
  redeems a session's refresh token; the others wait for the result.
- Roles are the client roles on the `melior` client
  (`resource_access.melior.roles`): `member` may join rooms, `admin` implies
  that and may manage every room. Realm roles are ignored.
- `redirectTo` and `state` only ever lead to app-relative paths.

## Rooms and access

A room is registered the first time someone joins it: open to every member,
owned by that person. The owner (or an admin) can make it invite-only.

| Path | What it does |
| --- | --- |
| `GET /api/rooms` | The rooms the caller owns or is invited to; all rooms for an admin. |
| `GET /api/rooms/{channel_id}` | One room. 404 if nobody registered the channel, 403 if it is invite-only and the caller is not invited. |
| `PUT /api/rooms/{channel_id}` | Body `{ "restricted": bool, "members": [username] }`. Registers the room for the caller if it does not exist; otherwise owner or admin only. |
| `DELETE /api/rooms/{channel_id}` | Owner or admin only. The channel is unregistered, and its chat history is deleted with it; whoever joins next owns it. |
| `GET /api/rooms/{channel_id}/messages?before=<id>&limit=<n>` | A page of the room's chat history, `{ "messages": [...], "has_more": bool }`, oldest first. Without `before` it is the newest page; with it, the messages before that id. `limit` is 1 to 100 (default 50). Same access as `GET /api/rooms/{channel_id}`: 404 for an unregistered channel, 403 for a room the caller may not enter. |

A room object is `{ channel_id, owner, restricted, members, is_owner }`.
Usernames are lower-case, 1 to 64 characters of `a-z`, `0-9`, `.`, `_`, `@`,
`-`. Requests that change something must come from the application's own
origin. Every open connection re-checks its access every
`SESSION_RECHECK_SECONDS`, so someone who loses access is removed within that
time.

## Signalling

`GET /ws?channel_id=<room>` upgrades to a WebSocket. The upgrade is refused
with 401 without a session, 403 without a role or an invitation (or from a
foreign origin), and 503 when another server instance hosts the room.

JSON messages of the form `{channel, payload}`:

- Server to client:
  - `session` (`session_id`, `channel_id`, `username`, `ice_servers`,
    `participants`), always first.
  - `peer_joined` (`session_id`, `username`), `peer_left` (`session_id`).
  - `sdp_answer` (`type`, `sdp`): the answer to the client's first offer.
  - `sdp_offer` (`type`, `sdp`, `tracks`): a renegotiation. `tracks` is the
    complete list of `{ mid, session_id, source }`: which media section carries
    whose track from now on, and whether it is that participant's `camera` or
    its `screen`.
  - `chat` (`id`, `username`, `text`, `sent_at_ms`, and `client_id` on the
    author's own copy only): someone said something in the room, the author
    included. `id` orders the room's messages.
  - `chat_error` (`message`, and the `client_id` of the refused message if it
    had one): a chat message the server did not accept. Only its sender gets it.
  - `ice`, `error` (`message`).
  - `closed` (`reason`): the last message before the server closes the socket.
    `session_ended` (log in again), `removed` (no access to this room any
    more), `room_moved` (reconnect).
- Client to server: `sdp_offer` (the first offer, and later one for each track
  the client adds or removes), `sdp_answer`, `ice` (an
  `RTCIceCandidateInit`), `echo`, `chat_send` (`text`, optional `client_id`).

### Screen sharing

A participant shares its screen by adding a second video track to its own
connection and sending an `sdp_offer` for the change. Nothing else is signalled:
the server works out what a track shows. A participant's first video track is its
camera, the next one is its screen, and a third is refused, so one participant
shows at most one screen at a time. Audio belongs to the camera, and a screen has
no sound. Everyone else receives the screen as one more track in their `tracks`
list, with `source: "screen"`. When the sharer stops, the track ends and the next
offer no longer lists it.

A client offer that crosses a server offer is ignored by the server. The client
gives its own up when the server's offer arrives, answers that, and offers its
change again once the connection is stable.

### Chat

A `chat_send` is checked, saved and sent to everyone in the room, the sender
included. The text is trimmed and must be 1 to 2000 characters with no control
characters other than line breaks and tabs. A participant may send 5 messages
per 10 seconds. A refused message gets a `chat_error` and nothing else happens.
Messages are stored in the `chat_messages` table, and the history endpoint above
reads them back, so people who join later see what was said. A room lives on one
instance (see [Several server instances](#several-server-instances)), so its
messages are delivered from that instance's memory; the store only orders and
keeps them.

The client makes the first offer. Once its media connection is established, the
server offers it the other participants' tracks, and renegotiates whenever
someone joins or leaves, one exchange at a time.

A media section outlives the track it was opened for: when a participant
leaves, the sections that carried their tracks go idle and carry the next
participant's tracks of the same codec, so a session description does not grow
with the number of people who have come and gone. Because WebRTC does not allow
a section's stream id to change, clients attribute media by the `tracks` list,
never by stream id. On a switch the server continues the section's RTP sequence
numbers and timestamps, so receivers see one uninterrupted stream.

## Configuration

### Server (`melior-sfu`)

| Variable | Default | Description |
| --- | --- | --- |
| `DATABASE_URL` | required | Room registry, `postgres://user:password@host:port/db`. Migrations run on first use |
| `MELIOR_BIND` | `0.0.0.0:5000` | Listen address |
| `MELIOR_INSTANCE_ID` | `melior-1` | Names this instance; must differ between instances sharing one Redis |
| `FRONTEND_URL` | `http://localhost:3000` | The origin browsers use for the app |
| `REDIS_URL` | `redis://127.0.0.1:6379` | Sessions and room leases, `redis://[:password@]host:port[/db]` |
| `IDP_ISSUER` | `http://localhost:8300/realms/melior` | OIDC issuer; endpoints and keys are discovered from it |
| `IDP_CLIENT_ID` | `melior` | Public client (PKCE, no secret) |
| `IDP_SCOPE` | `openid email profile` | Must include `openid`; `offline_access` is refused |
| `SESSION_COOKIE_NAME` | `melior_session_id` | |
| `SESSION_TTL_SECONDS` | `3600` | Session lifetime without a token refresh |
| `TOKEN_REFRESH_LEAD_SECONDS` | `30` | Refresh tokens this long before they expire |
| `SESSION_RECHECK_SECONDS` | `15` | How often an open connection re-checks its session, its room access and the room lease. A room lease lasts three times this |
| `MELIOR_TURN_URLS` | unset | Comma-separated ICE server URLs handed to browsers. The server itself connects directly |
| `MELIOR_TURN_SECRET` | unset | Secret shared with the TURN server; per-user credentials are derived from it |
| `MELIOR_TURN_TTL_SECONDS` | `3600` | Lifetime of a TURN credential |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | unset | OTLP gRPC endpoint for traces, e.g. `http://127.0.0.1:4317` |
| `RUST_LOG` | warnings, plus info for Melior's crates | Log filter (tracing) |

The server starts without Keycloak, Redis or Postgres being up; requests that
need them fail until they are. It stops on Ctrl-C or SIGTERM, flushing traces.

### Frontend

| Variable | Default | Description |
| --- | --- | --- |
| `PORT` | `3000` | Dev and preview server port |
| `MELIOR_SERVER_URL` | `ws://127.0.0.1:5000` | Backend the proxy forwards to |
| `MELIOR_SERVER_URLS` | unset | Several backends, comma separated; rooms are hashed over them |
| `ALLOWED_HOSTS` | unset | Extra host names the dev server answers to, comma separated (needed behind a reverse proxy) |
| `TLS_CERT` | `localhost.pem` | PEM certificate path |
| `TLS_KEY` | `localhost-key.pem` | PEM private key path |

HTTPS is used only if both files exist. To use the app from another device you
need HTTPS (for example a certificate from `mkcert`), a matching
`FRONTEND_URL`, that origin added to the Keycloak client's redirect URIs, and
TURN reachable from that device (below).

### Docker Compose (`docker/.env`)

Run `docker/init-env.sh` once. It creates `docker/.env` (mode 600) from
`.env.example` with random values for `POSTGRES_PASSWORD`,
`KEYCLOAK_DB_PASSWORD`, `KEYCLOAK_ADMIN_PASSWORD` and `TURN_SECRET`, and refuses
to overwrite an existing file unless you pass `--force`. Compose has no
defaults for these four and will not start without them. Postgres reads its
passwords only when its volume is first created; after changing them on an
existing stack run `docker compose down -v`, which deletes the databases and
the Keycloak realm. All other variables have development defaults;
`.env.example` lists them.

The test users live in `docker/keycloak/melior-users-0.json`, separate from the
realm. Set `KEYCLOAK_USERS_FILE=./keycloak/no-users.json` to import the realm
without them.

## TURN

coturn uses time-limited credentials (the TURN REST scheme, `use-auth-secret`).
For each participant the server derives a username `<expiry>:<user id>` and an
HMAC credential from `MELIOR_TURN_SECRET`, valid for `MELIOR_TURN_TTL_SECONDS`,
and hands them to the browser with the URLs in `MELIOR_TURN_URLS`.
`MELIOR_TURN_SECRET` must equal the `TURN_SECRET` coturn was started with;
compose wires both from `.env`. There is no static TURN user.

- By default coturn is published on `127.0.0.1` only (`COTURN_BIND`) with
  `TURN_EXTERNAL_IP=127.0.0.1`, which serves browsers on the same machine. For
  other machines set both to an address they can reach.
- The relay port range is 49152-49451, and an allocation nobody refreshes is
  freed after three minutes. Browsers allocate one relay port per network
  interface, so a machine with many interfaces uses many per tab; coturn logs
  `error 508` when the range is exhausted.
- To check the relay path, add `?relay=true` to a room URL: that browser then
  uses relay candidates only.
- TLS for coturn is off by default; see `docker/turnserver.conf`.

## Access over Tailscale

`docker/tailscale.sh up` makes a locally running stack reachable from every
device on your tailnet, over HTTPS with Tailscale's certificates (HTTPS must be
enabled for the tailnet):

- `tailscale serve` publishes the frontend on `https://<machine>.<tailnet>.ts.net:8443`
  and Keycloak on port `10000`. Port 443 is left alone.
- `docker/.env` gets the matching `FRONTEND_URL`, `KEYCLOAK_PUBLIC_URL`,
  `IDP_ISSUER` and `ALLOWED_HOSTS`, and coturn moves to the machine's tailnet
  address; Keycloak and coturn are restarted and the Keycloak client accepts the
  new origin.
- The script prints how to start the server and the frontend with those
  settings. `docker/tailscale.sh down` undoes all of it.

Media does not go through `tailscale serve`: browsers send it straight to the
machine's tailnet address over UDP, or through coturn on that address if they
cannot. While this is active everyone uses the tailnet URL, the machine itself
included; `http://localhost:3000` no longer logs in, because the session cookie
and the login redirect belong to the tailnet origin.

To allow some other origin on the Keycloak client of a running stack (a LAN
address, a reverse proxy), use `docker/keycloak-allow-origin.sh <origin>`.

## Several server instances

Conference state lives in the memory of the instance that hosts a room, so a
room is pinned to one instance:

- The proxy routes `/ws` by a consistent hash of `channel_id` over the
  instances in `MELIOR_SERVER_URLS`, so everyone in a room reaches the same
  instance. The other paths go to any instance.
- An instance takes a lease on a room in Redis before admitting anyone and
  renews it while participants are connected. An instance that does not hold
  the lease refuses the room with 503, so a room is never live on two
  instances, whatever the proxy does while instances come and go.
- A lease expires three re-check intervals after its instance stops renewing it
  (45 seconds by default). Clients reconnect on their own with backoff, and the
  next instance they reach takes the room over.
- Sessions (Redis) and the room registry (Postgres) are shared, so a user stays
  logged in whichever instance answers.

Every instance needs its own `MELIOR_INSTANCE_ID`. There is no media relay
between instances: a room's participants all send to and receive from the
instance that hosts it.

## Compose profiles

- (none): `postgres`, `redis`, `keycloak`, `coturn`, `jaeger`, all published on
  `127.0.0.1`.
- `app`: also builds and runs `melior-server` (host network, port 5000) and
  `melior-frontend` (nginx, port 3000). Linux only, because the SFU needs host
  networking for its UDP ports.

  ```sh
  cd docker
  docker compose --profile app up --build
  ```

- `scale`: adds `melior-server-2` (port 5001, instance id `melior-2`). Start
  with `--profile app --profile scale` and set
  `MELIOR_SERVER_URLS=http://host.docker.internal:5000,http://host.docker.internal:5001`.
- Production override:
  `docker compose -f docker-compose.yaml -f docker-compose.prod.yaml --profile app up -d`
  runs Keycloak in production mode (`start`) without the test users and
  requires `KEYCLOAK_PUBLIC_URL`, `FRONTEND_URL`, `IDP_ISSUER`, `COTURN_BIND`
  and `TURN_EXTERNAL_IP`. It assumes a reverse proxy that terminates TLS in
  front of Keycloak and the frontend. The file's header lists what it does not
  provide.

Traces are in Jaeger at `http://localhost:16686`, service `melior-sfu`.

## Releases

A release is a SemVer tag on `main`:

```sh
git switch main && git pull
git tag vX.Y.Z          # or vX.Y.Z-rc.1 for a pre-release
git push origin vX.Y.Z
```

`.github/workflows/release.yml` checks that the tag is SemVer and on `main`,
runs the full checks, then publishes two linux/amd64 images to GitHub Container
Registry and creates the GitHub Release with generated notes:

| Image | Built from |
|---|---|
| `ghcr.io/jonohas/melior-server:X.Y.Z` | `melior-server/Dockerfile` |
| `ghcr.io/jonohas/melior-frontend:X.Y.Z` | `melior-frontend/Dockerfile` |

Tags carry no leading `v`. Both images also get `:latest`, except for a
pre-release (`-rc.1`, ...), which is published under its own version only and
marked as a pre-release on GitHub. The images carry the OCI labels `source`,
`version` and `revision`.

The first time, GHCR may create the packages as private. Make both public in
the package settings (Package settings, Danger Zone, Change visibility) so the
deploy host can pull without credentials.

Deployment is not part of this repository: the infrastructure repository
(`Jonohas/infra`) pins the version to run and rolls it out.

## Tests and checks

```sh
cd melior-server
cargo test --workspace --locked      # unit tests per crate; HTTP tests for login and rooms; forced-close
                                     # tests; end-to-end tests with real WebRTC clients. Needs no
                                     # Keycloak, Redis, Postgres or TURN: fakes sit behind the ports.
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items --locked
cargo xtask layers                   # the dependency rules between the crates

cd melior-frontend
bun run test:unit && bun run check-types && bun run biome-check && bun run build
```

CI runs the same commands (`.github/workflows/checks.yml`). The Redis,
OIDC, TURN and the Redis and room-registry adapters are exercised only by running
the stack; see `AGENTS.md` section 5. The chat history adapter has an ignored test
that needs a database:

```sh
MELIOR_TEST_DATABASE_URL=postgres://user:password@127.0.0.1:5432/db \
    cargo test -p melior-infrastructure --test postgres_chat -- --ignored
```

## Limitations

- The realm in `docker/keycloak` is a development realm: `localhost` redirect
  URIs, and test users with known passwords unless `KEYCLOAK_USERS_FILE` says
  otherwise.
- The production override does not provide TLS termination, backups, Keycloak
  clustering or a hardened realm, and it has only been checked by starting it
  locally.
- One Postgres, one Redis, one Keycloak and one coturn: the Compose stack has
  no high availability.
- No media relay between server instances. When an instance stops, its rooms
  are interrupted until their lease expires and the participants reconnect.
- A participant who loses access or whose session ends is disconnected at the
  next re-check, up to `SESSION_RECHECK_SECONDS` later, not instantly.
- A media section is reused only for a track of the same codec, so a room with
  mixed codecs keeps one set of sections per codec.
- The server needs host networking in Docker, so the `app` profile works on
  Linux only.
- There is no recording, screen sharing, simulcast or bandwidth adaptation.
