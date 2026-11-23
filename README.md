# Melior

Melior is a video-conferencing system. Signed-in users meet in communities
(the UI calls them servers, as Discord does): each has text channels and voice
channels, roles and invites. A voice channel is a conference room in the
browser. Rooms of up to `MELIOR_MESH_MAX_PARTICIPANTS` people send audio and
video browser to browser; bigger rooms use a selective forwarding unit (SFU)
written in Rust, which receives each participant's audio and video and forwards
it to the others in the same room. A room switches between the two as people
come and go, without freezing any video.

## Architecture

- `melior-server/` - the SFU (binary `melior-sfu`), built on axum and webrtc-rs.
  It also acts as the backend-for-frontend (BFF) for login: it runs the OpenID
  Connect flow against Keycloak and keeps sessions in Redis, so tokens never
  reach the browser. Communities (roles, members, channels, permission
  overrides, invites) and chat history are stored in Postgres; who is in which
  voice channel and the community events travel through Redis. Media flows over WebRTC on arbitrary UDP ports. In a mesh room (see
  "Mesh" below) the server only signals: media goes straight between browsers,
  with TURN as the fallback, and the SFU forwards nothing for that room.
- `melior-frontend/` - a Bun workspace run by Turborepo with one app,
  `apps/web`: TanStack Start (SPA mode) on Vite, React 19, Tailwind 4. In
  development Vite proxies the backend paths; in production nginx does.
- `docker/` - `docker-compose.yaml` with Postgres, Redis, Keycloak, coturn
  (TURN/STUN), Jaeger and RustFS (chat images), and optionally the application services.

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
melior-domain          L0  conference/  Room aggregate, participants, tracks, negotiation rules, mesh
                                        target, RoomPlan (media mode, capacity), VoiceState
                           community/   Community, roles, members, categories, channels, overrides,
                                        invites; Permissions, the permission resolution (Standing)
                                        and the hierarchy rules
                           identity/    User and roles, Username, session id, post-login return path
melior-application     L1  conference/  ConferenceService (join, leave, offer, answer, ICE, track events,
                                        voice states, hosting); ports MediaEngine, MediaSession,
                                        MediaEvents, SignalSink, IceServerProvider, RoomLeases,
                                        PresenceSink
                           communities/ CommunityService (structure, members, roles, invites, voice
                                        admission, feed filter); ports CommunityRepository,
                                        CommunityEvents, VoiceRoster; PresenceQueue
                           auth/        AuthService (login round trip, session checks with token
                                        refresh, logout); ports IdentityProvider, SessionStore
                           chat/        ChatService (room chat, text channel posts, history); port ChatRepository
melior-infrastructure  L2  webrtc/      webrtc-rs media engine: peer connections, RTP forwarding with
                                        reusable outlets, keyframe requests
                           oidc/        OpenID Connect identity provider (code flow with PKCE, verification)
                           redis/       session store, token-refresh claims, room leases, community
                                        events (pub/sub), voice roster
                           postgres/    communities and chat history on sqlx, with embedded migrations
                           turn.rs      per-user, expiring TURN credentials
                           telemetry.rs logging and OpenTelemetry trace export
                           config.rs    environment configuration
melior-interfaces      L2  http/        /login, /signin, /signout, /api/identity/me, /api/health, and the
                                        community API (/api/communities, /api/channels, /api/invites)
                           ws/          the signalling WebSocket and the community feed: wire-format
                                        DTOs and connection handlers
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
`lib/auth` (login gate and session loading), and three features,
`features/communities`, `features/conference` and `features/chat`, each split into
`{domain,api,hooks,components}`: pure TypeScript domain, browser adapters, the
application hooks, and presentation. See `melior-frontend/README.md`.

## Prerequisites

- Rust (the version in `melior-server/rust-toolchain.toml`; rustup installs it)
- Bun
- Docker with Compose
- A browser. Camera and microphone access requires HTTPS or `localhost`.

## Local development

1. Create the secrets and start the infrastructure (Postgres, Redis, Keycloak,
   coturn, Jaeger, RustFS):

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
   S3_ENDPOINT=http://127.0.0.1:9000 S3_ACCESS_KEY="$RUSTFS_ACCESS_KEY" S3_SECRET_KEY="$RUSTFS_SECRET_KEY" \
   cargo run
   ```

   Only `DATABASE_URL` is required; TURN, trace export, image uploads (`S3_*`)
   and GIF search (`KLIPY_APP_KEY`) are optional.

   For anything beyond a quick look, run `cargo run --release`. The media
   path (SRTP, RTP forwarding) is several times slower in a debug build, which
   shows up as late video and audio as soon as the machine is busy.

3. Start the frontend:

   ```sh
   cd melior-frontend
   bun install
   bun dev
   ```

4. Open `http://localhost:3000`, sign in, create a server (or join one with an
   invite code) and open a voice channel. To see two participants
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
  (`resource_access.melior.roles`): `member` or `admin` may use Melior (create
  and join communities); a user with neither is refused everywhere. The `admin`
  role gives no powers inside a community. Realm roles are ignored.
- `redirectTo` and `state` only ever lead to app-relative paths.

## Communities

A community (called a server in the UI) has members, roles, categories and
channels. A **voice channel** is a conference room with its own chat; a **text
channel** holds messages only. Any user with a Melior role may create a
community and becomes its owner; others join through an invite. A channel is
never created by joining it: an unknown channel is refused.

### Permissions

Permissions are bits with wire names: `view_channel`, `connect`,
`send_messages`, `read_history`, `create_invite`, `manage_channels`,
`manage_roles`, `kick_members`, `administrator`. A new community's `@everyone`
role (its id equals the community id) has `view_channel`, `connect`,
`send_messages`, `read_history` and `create_invite`. A category or channel may
carry overrides: per role or per member, permissions to allow and to deny.

What a member may do in a channel is resolved in this order:

1. The owner has every permission everywhere.
2. Base = `@everyone`'s permissions plus those of each of the member's roles.
3. With `administrator` in the base, the member has every permission.
4. A channel that is `synced` and in a category uses the category's overrides,
   otherwise its own.
5. `@everyone`'s override removes its deny and adds its allow; then the union of
   the member's roles' denies is removed and the union of their allows added;
   then the member's own override, deny then allow.
6. Without `view_channel`, nothing is left.

Community-wide actions (`create_invite`, `manage_channels`, `manage_roles`,
`kick_members`, renaming) use the base permissions (steps 1 to 3). Renaming
needs `administrator` (or the owner); deleting is for the owner alone.

Hierarchy: a member's rank is the highest position of their roles (0 without
any); the owner outranks everyone. Creating, editing or deleting a role, and
assigning or unassigning it, needs `manage_roles` and the role (its old and new
position) below the actor's rank; a role cannot be given permissions the actor
lacks (unless owner or administrator); only permissions a change adds are
checked. A new role is placed just below the actor's rank, positions may be
shared. Kicking needs `kick_members` and a target ranked below the actor, never
the owner. Everyone but the owner may leave; the owner may only delete the
community. Editing a channel's overrides unsyncs it from its category; setting
`synced` again restores the category's overrides.

### Media mode and user limit

A voice channel has a `media_mode` (`auto`, `peer_to_peer`, `server`) and an
optional `user_limit` (2 to 99). With this instance's mesh limit (see Mesh):

| Mode | Room | Capacity |
| --- | --- | --- |
| `server` | always forwarded | `user_limit` |
| `peer_to_peer`, mesh on | always direct (a failed link still pins the room forwarded until the next join or leave) | `min(user_limit or mesh limit, mesh limit)`; `limit_clamped` when the limit is above the mesh limit |
| `peer_to_peer`, mesh off | forwarded, warning `mesh_off` | `user_limit` |
| `auto`, `user_limit` at most the mesh limit | as `peer_to_peer` | `user_limit` |
| `auto` otherwise | switches with the room's size, as described under Mesh | `user_limit` |

Saving a `peer_to_peer` channel with a `user_limit` above the mesh limit is
refused (above 8 when mesh is off). A text channel has no media settings. The
channel view reports `effective_mode`, `effective_limit` and `media_warning`. A
room takes its channel's plan at every join; a full room refuses the join.

### Community API

All bodies are JSON; invalid input is a 400 with a plain-text reason. 401
without a session, 403 without a Melior role, for a non-member or for a missing
permission (with the reason as text), 404 for something unknown, 503 when the
store cannot be reached. Requests that change something must come from the
application's own origin.

| Path | What it does |
| --- | --- |
| `GET /api/communities` | `[{ id, name, owner_username }]`, the caller's communities. |
| `POST /api/communities` `{ name }` | Creates one: the caller owns it; it starts with `Text channels`/`general` and `Voice channels`/`General`. Returns the community view. |
| `GET /api/communities/{id}` | The community view: `{ id, name, owner_id, owner_username, me: { user_id, role_ids, permissions, rank, is_owner }, roles, categories, channels }`. `channels` holds only the channels the caller may view, by position. |
| `PATCH /api/communities/{id}` `{ name }` | Owner or `administrator`. |
| `DELETE /api/communities/{id}` | Owner only; deletes the channels and their history. 204. |
| `GET /api/communities/{id}/members` | `[{ user_id, username, role_ids }]`. |
| `DELETE /api/communities/{id}/members/{user_id}` | Kick, or leave when it is the caller. 204. |
| `PUT /api/communities/{id}/members/{user_id}/roles` `{ role_ids }` | Replaces the member's roles. |
| `POST /api/communities/{id}/roles` `{ name, color?, permissions }` | A role just below the caller's rank. |
| `PATCH /api/communities/{id}/roles/{role_id}` `{ name?, color?, permissions?, position? }` | `@everyone` changes its permissions only. |
| `DELETE /api/communities/{id}/roles/{role_id}` | Not `@everyone`. 204. |
| `POST /api/communities/{id}/categories` `{ name }` | `manage_channels`. |
| `PATCH /api/communities/{id}/categories/{category_id}` `{ name?, position?, overrides? }` | |
| `DELETE /api/communities/{id}/categories/{category_id}` | Its channels stay, without a category. 204. |
| `POST /api/communities/{id}/channels` `{ name, kind, topic?, category_id?, media_mode?, user_limit? }` | `manage_channels`; returns the channel view. |
| `GET /api/channels/{channel_id}` | The channel view: `{ id, community_id, name, topic, kind, category_id, position, synced, overrides, media_mode, user_limit, effective_mode, effective_limit, media_warning, my_permissions }`. |
| `PATCH /api/channels/{channel_id}` `{ name?, topic?, category_id?, position?, media_mode?, user_limit?, overrides?, synced? }` | `category_id: null` takes it out of its category, `user_limit: null` removes the limit. |
| `DELETE /api/channels/{channel_id}` | With its history. 204. |
| `GET /api/channels/{channel_id}/admission` | 204 if the caller may join the voice channel now, else the status the WebSocket upgrade would give (403, 404, 409 full by the community's voice roster). For clients, which cannot see a refused upgrade's status. |
| `GET /api/channels/{channel_id}/messages?before=<id>&limit=<n>` | `{ "messages": [...], "has_more": bool }`, oldest first; `limit` 1 to 100 (default 50). Needs `view_channel` and `read_history`. |
| `POST /api/channels/{channel_id}/messages` `{ text, client_id?, attachment? }` | Text channels only (409 for voice). Needs `view_channel` and `send_messages`; 429 over the rate limit. `text` may be `""` with an attachment (see [Chat](#chat)). Returns the message with the `client_id`; the others get it on the feed. |
| `POST /api/channels/{channel_id}/attachments` (body: the image file) | Uploads an image for a message in a text or voice channel: `{ id, content_type, size }`. Needs `view_channel` and `send_messages`. PNG, JPEG, GIF or WebP by content, at most 10 MiB (413 above); 400 otherwise, 429 over 10 uploads a minute, 503 when the server has no image storage. |
| `GET /api/attachments/{id}` | The image, with its media type, `nosniff` and a private cache lifetime of a year. Needs `view_channel` and `read_history` in the channel it was uploaded to; 404 for an unknown id. |
| `GET /api/gifs/config` | `{ "klipy_app_key": string \| null }` for signed-in users: the key browsers search KLIPY with; `null` turns GIFs off. |
| `POST /api/communities/{id}/invites` `{ max_age_seconds?, max_uses? }` | `create_invite`. Lifetime 1800, 3600, 21600, 43200, 86400 or 604800 seconds, or none (never expires); uses 1 to 100 or none. |
| `GET /api/communities/{id}/invites` | `manage_channels`. |
| `GET /api/invites/{code}` | `{ code, community: { id, name }, member_count, usable }` for anyone with a Melior role. |
| `POST /api/invites/{code}` | Joins: `{ community_id }`. A member gets the id again without using the invite; an expired or used-up invite is 410. |
| `DELETE /api/invites/{code}` | Its creator, or `manage_channels`. 204. |

An override is `{ "target": { "kind": "role" | "member", "id": ... }, "allow": [...], "deny": [...] }`;
an invite `{ code, community_id, created_by, expires_at_ms, max_uses, uses }`.
An invite code is 10 characters of `A-Z`, `a-z`, `0-9`. Redeeming is one
conditional update, so two people cannot both take an invite's last use.

Every open connection re-checks its access every `SESSION_RECHECK_SECONDS`, so
someone who is kicked, loses a permission or whose channel is deleted is removed
within that time.

### Community feed

`GET /ws/communities/{id}` is a server-to-client WebSocket of
`{channel, payload}` messages; what the client sends is ignored. The upgrade
needs the own origin, a session and membership (403; 404 for an unknown
community).

| `channel` | `payload` |
| --- | --- |
| `snapshot` | `{ "voice": [VoiceEntry] }`: first, and again after every re-check. Replaces what the client knew |
| `voice_joined`, `voice_updated` | `VoiceEntry` |
| `voice_left` | `{ channel_id, session_id }` |
| `message` | `{ channel_id, message }`, a new message in a text channel |
| `structure_changed` | `{}`: fetch `GET /api/communities/{id}` again |
| `members_changed` | `{}`: someone joined through an invite, left or was kicked; fetch `GET /api/communities/{id}/members` again |
| `removed` | `{ reason: "kicked" \| "left" \| "deleted" }`, then the server closes |
| `closed` | `{ reason: "session_ended" }`, then the server closes |

A `VoiceEntry` is `{ channel_id, session_id, user_id, username, muted, camera,
screen, speaking }`. The feed carries entries and messages only for channels
the member may view. Presence is kept in Redis: one key per participant that
lives three re-check intervals and is renewed on every re-check, so the
participants of an instance that stopped disappear with the next snapshot.
Events travel over Redis pub/sub (`melior:community:<id>`), one subscription per
instance.

### Existing rooms

Migration `0003_communities` turns the `rooms` table into `channels`. If any
rooms exist, they all join one community named `Melior`, owned by the owner of
the oldest room: every room owner becomes a member, `@everyone` gets the
default permissions, and the community gets one invite that never expires (see
`GET /api/communities/{id}/invites`). Each room becomes a voice channel named
after its channel id, without a category, in mode `auto`, keeping its chat
history. A restricted room is hidden from `@everyone` and shown to its owner by
overrides. Invitation lists are dropped: they held usernames, not user ids, so
invited users rejoin through the invite.

## Signalling

`GET /ws?channel_id=<id>` upgrades to a WebSocket in a voice channel. The
upgrade is refused with 400 without a valid `channel_id`, 401 without a
session, 403 without a role, for a non-member, without `view_channel` or
`connect`, for a text channel (or from a foreign origin), 404 for an unknown
channel, 409 when the channel is full, and 503 when another server instance
hosts the room. A browser cannot see these statuses; it asks
`GET /api/channels/{channel_id}/admission`.

JSON messages of the form `{channel, payload}`:

- Server to client:
  - `session` (`session_id`, `channel_id`, `username`, `ice_servers`,
    `participants`), always first. Each participant is `{ session_id,
    username, voice }`, and `voice` is `{ muted, camera, screen, speaking }`.
  - `peer_joined` (`session_id`, `username`, `voice`), `peer_left` (`session_id`).
  - `voice_state` (`session_id`, `muted`, `camera`, `screen`, `speaking`): a
    participant's voice state changed; everyone in the room gets it, the
    participant included.
  - `sdp_answer` (`type`, `sdp`): the answer to the client's first offer.
  - `sdp_offer` (`type`, `sdp`, `tracks`): a renegotiation. `tracks` is the
    complete list of `{ mid, session_id, source }`: which media section carries
    whose track from now on, and whether it is that participant's `camera` or
    its `screen`.
  - `chat` (`id`, `username`, `text`, `sent_at_ms`, `attachment` if the
    message has one, and `client_id` on the author's own copy only): someone
    said something in the room, the author included. `id` orders the room's
    messages. `text` is `""` for a message with only an attachment.
  - `chat_error` (`message`, and the `client_id` of the refused message if it
    had one): a chat message the server did not accept. Only its sender gets it.
  - `ice`, `error` (`message`).
  - `route` (`peer`, `path`, `epoch`): play `peer`'s media over `path`
    (`direct` or `forwarded`) as soon as it works there, then report
    `route_live`. For `direct`, open the direct link to `peer` if it does not
    exist.
  - `link_close` (`peer`): close the direct link to `peer`.
  - `uplink` (`active`): pause (`false`) or resume (`true`) sending on the SFU
    connection.
  - `peer_offer` (`from`, `sdp`), `peer_answer` (`from`, `sdp`): the relayed
    offer and answer of the direct link with `from`.
  - `peer_ice` (`from`, `candidate`): a relayed ICE candidate for that link.
  - `e2ee` (`from`, `payload`): an end-to-end key message from `from`,
    relayed unread (see [End-to-end encryption](#end-to-end-encryption)).
  - `closed` (`reason`): the last message before the server closes the socket.
    `session_ended` (log in again), `removed` (no access to this channel any
    more: kicked, a permission lost, or the channel deleted), `room_moved`
    (reconnect).
- Client to server: `sdp_offer` (the first offer, and later one for each track
  the client adds or removes), `sdp_answer`, `ice` (an
  `RTCIceCandidateInit`), `echo`, `chat_send` (`text`, optional `client_id` and `attachment`),
  `voice_state` (`muted`, `camera`, `screen`, `speaking`, all booleans), and:
  - `route_live` (`peer`, `epoch`): I now play `peer`'s media over the path of
    route `epoch`, or I already did when the route arrived.
  - `peer_offer` (`to`, `sdp`), `peer_answer` (`to`, `sdp`): the offer and
    answer for the direct link with `to`.
  - `peer_ice` (`to`, `candidate`): an ICE candidate for that link.
  - `e2ee` (`to`, `payload`): an end-to-end key message for another
    participant of the room. `payload` is 1 to 4096 characters of unpadded
    base64url; anything else, or a `to` outside the sender's room, is dropped
    without an `error`. Unlike `peer_*` it needs no direct link, and it has
    a rate limit of its own (64 at once, then 16 per second), so a burst of
    ICE candidates cannot starve the key exchange.
  - `link_failed` (`peer`): my direct link to `peer` reached `failed`.

Every `peer` (and `from`, `to`) is a session id. The server drops a relayed SDP
over 64 KiB and a candidate over 1 KiB.

A `voice_state` is a claim the room checks: a muted participant is not
speaking, and `screen` counts only while the room records a screen track of
the participant, or while its uplink is paused (every link direct: the screen
then travels over the direct links only and the SFU never sees its track). An
unchanged state is not sent again. At most 10 updates per second are taken from
one participant; the rest are dropped. Every change also goes to the
community feed.

### Screen sharing

A participant shares its screen by adding a second video track to its own
connection, and a second audio track when it shares the screen's sound, and
sending an `sdp_offer` for the change. Nothing else is signalled: the server
works out what a track shows, per kind. A participant's first audio and first
video track are its microphone and camera, the next one of each kind is its
screen's sound and picture, and a third of a kind is refused, so one participant
shows at most one screen at a time. The client adds the screen's tracks only
after its connection is up, so the microphone and camera are always published
first. Everyone else receives the screen's tracks as more entries in their
`tracks` list, with `source: "screen"`. When the sharer stops, the tracks end and
the next offer no longer lists them. A screen share is sent on the server
connection. In a mesh room it also travels over each direct link, in fixed third
(screen video) and fourth (screen audio) media sections, after audio and camera
video, that the link negotiates once. Starting and stopping the share uses
`replaceTrack` on those sections' senders, so no direct link is renegotiated.

The browser's screen picker decides whether the sound is shared: the person
ticks "Share audio" (Chromium shares a tab's sound everywhere and the whole
system's sound on Windows and ChromeOS). The client asks for it without echo
cancellation, noise suppression or gain control, and plays a screen's sound
next to the sharer's voice. The sharer tells each peer with `{"screen": true}` or
`{"screen": false}` on the link's `melior` data channel, and the peer shows the
screen from the direct path only while the last state it got is `true`. On
forwarded links the screen travels as before, as a track in `tracks`. The paused
uplink covers the screen sender like the others.

A client offer that crosses a server offer is ignored by the server. The client
gives its own up when the server's offer arrives, answers that, and offers its
change again once the connection is stable.

### Chat

A `chat_send` is checked, saved and sent to everyone in the room, the sender
included. It needs `send_messages` in the channel; without it the sender gets a
`chat_error` saying "you may not send messages in this channel". The text is trimmed and must be 1 to 2000 characters with no control
characters other than line breaks and tabs. A participant may send 5 messages
per 10 seconds. A refused message gets a `chat_error` and nothing else happens.
Messages are stored in the `chat_messages` table, and
`GET /api/channels/{channel_id}/messages` reads them back, so people who join later see what was said. A room lives on one
instance (see [Several server instances](#several-server-instances)), so its
messages are delivered from that instance's memory; the store only orders and
keeps them.

A message may carry one attachment besides its text, and its text may then be
empty. A client sends it as `attachment`, and saved messages carry it the same
way, in the room, the history and the feed:

| `attachment` | Sent | Saved |
| --- | --- | --- |
| Image | `{ "kind": "image", "id": <id> }`, the id from `POST /api/channels/{channel_id}/attachments` | `{ "kind": "image", "id", "content_type" }`; the client loads `GET /api/attachments/{id}` |
| GIF | `{ "kind": "gif", "url", "width", "height" }` | the same |

Only the uploader may attach an image, and only in the channel it was uploaded
to; any other id is refused as "the image was not found, upload it again". A
GIF's `url` must be `https` on `static.klipy.com`, `static1.klipy.com` or
`static2.klipy.com`, and each side 1 to 4096; otherwise the refusal is "a GIF
must come from KLIPY". The image bytes live in S3-compatible storage (RustFS in
the compose stacks) under `chat-images/<id>`, and their record in the
`chat_images` table. The server creates the bucket on the first upload. Deleting
a channel deletes its image records, but the objects stay in the bucket.

An uploaded image starts out not attached; the message that carries it attaches
it. Every server instance sweeps images that stay unattached longer than
`UNATTACHED_IMAGE_TTL_SECONDS`, record first, then the object, up to 200 per
sweep. Instances take different images, and an image a message attaches during
a sweep is kept. Once swept, sending the image is refused as "the image was not
found, upload it again".

GIFs come from [KLIPY](https://docs.klipy.com/); Tenor's API was shut down on
30 June 2026. KLIPY requires searches to come from the browser and its media to
be loaded from its own servers, so signed-in browsers get the app key from
`GET /api/gifs/config` and search KLIPY directly. The key is an app key for a
public API, not a credential of the user. It is the one key besides the e2ee
token that reaches the browser. The search box says "Search KLIPY", as KLIPY's
attribution rules require.

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

### Lip sync

A browser plays the audio and video sections that announce one stream id in
step, and holds the earlier back to match the later. Each participant's
microphone and camera share one stream id, and so do its screen's picture and
sound. Nobody else's sections share it, so nobody's video waits for someone
else's audio. This holds on both paths:

- **Forwarded.** A section's stream id cannot change, so the server gives a
  track to a section that already has its group's stream id, or to one whose
  stream id no section in use carries, or opens a section
  (`melior_domain::choose_section`). The server passes each publisher's own RTCP
  sender reports on to the receivers, with the timestamps rewritten as the
  section's. It makes no reports of its own: those would pair each RTP time
  with the moment the packet passed through the SFU, which hides how much later
  than its sound a busy publisher sent a frame. A simulcast layer switch keeps
  the section on the same clock.
- **Direct.** Each side's sections announce the same layout from the first
  offer: the local stream for the microphone and camera, and a second stream for
  the screen and its sound. The answerer sets them with `setStreams` before it
  answers. A browser without `setStreams` sends its answered sections without a
  stream, and its peer then plays that sound and picture without lining them up.

After a section moves to another participant, the receiver learns the new
clock from the next sender reports, about a second later.

Audio is sent as RED (RFC 2198) where the browser has it: every packet repeats
the frame before it, so one lost packet costs no sound. Audio has no
retransmission, and the sender's Opus FEC only reacts to loss on its own uplink.
The client puts RED first on its audio sections and sends audio at high
priority. The server forwards RED to a participant whose connection negotiated
it, with the blocks renumbered to that participant's Opus payload type, and only
the current frame, as plain Opus, to one whose connection did not.

### Mesh

Small rooms send media directly between browsers; the server decides when, and
the browsers switch without a gap.

- **Paths.** A *direct* path is a browser-to-browser `RTCPeerConnection`; a
  *forwarded* path goes through the SFU. The SFU connection stays open for the
  whole session, whatever the path.
- **Target.** The path the room as a whole wants, `direct` or `forwarded`. The
  server re-evaluates it after every join and leave, with hysteresis. With the
  default limit of 4, a 5th participant moves the room to `forwarded`, and
  dropping to 3 moves it back to `direct`; at exactly 4 the room keeps what it
  had. With a limit of `0` the target is always `forwarded`.
- **Links and routes.** Every pair of participants has one link, created when
  the second of them joins and removed when either leaves. A link's *route* is
  `Forwarded`, `ToDirect`, `Direct` or `ToForwarded`, and has an `epoch` that
  grows on every change. A new link starts as `Forwarded`, so a newcomer sees
  everyone through the SFU at once. A `route_live` for any other epoch is
  ignored.
- **Make before break.** On `route`, both browsers set up the new path while the
  old one keeps playing. Each shows the new path once its media is flowing
  there and reports `route_live`. A browser that already plays the peer's
  media on the route's path reports at once. Only when both have reported does the server
  stop the old path: it unsubscribes both from each other's SFU tracks
  (`ToDirect` to `Direct`) or sends `link_close` (`ToForwarded` to `Forwarded`).
  Until then both paths carry media. Switching is per pair.
- **Paused uplink.** A participant's SFU uplink is paused when it has at least
  one link and every link is `Direct`; the server sends `uplink` when this
  changes. The track stays published, the sender only stops sending packets.
  A participant alone in a room keeps sending.
- **Devices and mute.** Switching camera or microphone replaces the track on
  the server connection and on every direct link before the old track is
  stopped. Mute applies to the shared local tracks, so it holds on every
  connection.
- **Relay rule.** The server relays `peer_offer`, `peer_answer` and `peer_ice`
  only when the target is in the sender's room and their link is `ToDirect`,
  `Direct` or `ToForwarded`. Anything else is dropped without an `error`, because
  a late ICE candidate after `link_close` is normal.
- **Roles.** The browser whose session id sorts lower makes the offer on a
  direct link; the other answers.
- **When a direct link fails.** Two things count as a failure: a link in
  `ToDirect` that both browsers have not reported within 10 seconds, and a
  `link_failed` from either browser for a link in `ToDirect`, `Direct` or
  `ToForwarded`. A browser sends `link_failed` when its direct connection
  reaches `failed`, and does nothing else (no whole-session reconnect). The
  server then pins the room to `forwarded`, and every link moves back through
  the SFU: a `Direct` link gets its SFU subscriptions back and goes to
  `ToForwarded`, and the others follow the target. The pin lasts until the next
  join or leave. A `link_failed` for a `Forwarded` link, an unknown peer or a
  peer outside the sender's room is ignored.

### Media encoding

Every connection negotiates VP8 for video and Opus for audio, nothing else:
the client puts VP8 first with `setCodecPreferences`, and the server registers
only these two codecs, so a section offered with another codec gets no codec
in the answer. Nothing caps quality by default: under **Audio and video**,
each person chooses a camera quality (**Best the camera offers**, or up to
1080p, 720p, 480p or 360p) and a screen sharing smoothness (**Smoothest
available**, or up to 60, 30, 15 or 5 frames per second). The choice applies at
once to the camera and to a screen being shared, and is remembered in the
browser (`melior.quality`). No bitrate is capped; the browser's congestion
control fits what is sent to the network. The client tells the encoder what each track shows
(`contentHint`: `motion`, `detail`, `speech`).

| Sender | Path | Settings |
|---|---|---|
| Camera | SFU | simulcast: `q` (1/4 size), `h` (1/2), `f` (full); no bitrate or frame-rate cap |
| Camera | direct link | none: the browser's congestion control adapts the one stream to the one receiver |
| Screen | both | `degradationPreference: maintain-resolution`: short of bandwidth, it drops frames before sharpness |
| Microphone | both | the browser's defaults |

The SFU forwards one layer of a simulcast camera to each receiver. Every
receiver gets the full layer, however big the room grows: room size never
lowers quality. Only a receiver's own network does (`ReceiverLayer`): after 2
receiver reports in a row with more than 10% loss it gets the next layer down,
and after 10 reports in a row with at most 2% loss the next layer up. If a
layer stops arriving (the browser's
congestion control drops top layers on a short uplink), it forwards the best
layer below it, else the lowest above it. A receiver moves to another layer
only at that layer's next keyframe, which the SFU asks for; the VP8 header that
marks a keyframe stays in the clear under end-to-end encryption. Screens,
microphones and direct links send a single stream.

One writer per connection applies these settings together with the paused
uplink (`uplink`), after every negotiation, because a second `setParameters`
on a sender fails while one runs. The design these settings come from, and
what is still to be built, is in [`docs/media-pipeline.md`](docs/media-pipeline.md).

### End-to-end encryption

Every call is encrypted end to end: the SFU, TURN and the network carry media
they cannot decode. A browser without `RTCRtpScriptTransform` (older than
Chrome and Edge 141, Firefox 117 or Safari 15.4) does not join.

- **Frames.** Each browser encrypts every encoded frame it sends with its own
  AES-GCM media key, in a worker (`RTCRtpScriptTransform`), before the frame is
  packetized. The codec header stays in the clear (VP8: 10 bytes of a
  keyframe, 3 of a delta frame; Opus: 1 byte), so receivers depacketize as
  usual and the SFU can tell keyframes. A trailer names the key (a random
  32-bit id) and the frame counter, which with the key id form the nonce. A
  receiver drops any frame it cannot decrypt. Direct links are encrypted the
  same way, so a switch between paths changes nothing for the receiver.
- **Keys.** Each browser has a device key (ECDSA P-256), created once and kept
  non-extractable in IndexedDB, and a fresh ECDH P-256 key per call. On joining,
  it sends every participant a `hello` with both public keys and a device-key
  signature over the call key and both session ids. When a hello verifies,
  both sides derive a pair key (ECDH, then HKDF-SHA-256) and send each other
  their media key wrapped with it. The server relays these messages and holds
  no key that opens them.
- **Rotation.** On every join and leave, each browser makes a new media key,
  sends it to everyone present, and encrypts with it one second later.
  Someone who left gets no new key; someone who joined never had the old one.
- **Vouching.** Keycloak vouches for each device key, so a server that swaps
  keys is caught without anyone comparing a code. Before a call starts, a
  browser without a token (or with one in its last hour) goes to the realm's
  `melior-e2ee` client in a full-page redirect with `prompt=none`: with a live
  Keycloak session nothing is shown, and the browser comes back to
  `/e2ee-callback`, then to the call. There it trades the code for an access
  token bound to its device key with DPoP (`cnf.jkt` is the key's thumbprint),
  and keeps it in `localStorage` (`melior.e2ee.certificate`). A round trip that
  brings no token is not repeated for 10 minutes. The browser sends the token
  in its hello.
  Each peer checks it against the realm's published keys, fetched from the
  realm, never through the Melior server: signed by the realm, issued to
  `melior-e2ee`, not expired, bound to the hello's device key, and naming the
  participant's username. Only then does it send its media key. A participant
  the realm does not vouch for gets no key and sees nothing; the badge then
  says **Encrypted · unverified devices**, also on that participant's own
  screen. The token grants nothing (the client has no roles), lasts 12 hours,
  and is useless without the device's private key. The browser reads the
  realm's address from `/e2ee.json`, which the frontend serves (`E2EE_ISSUER`
  below); without it there is no vouching: keys flow to everyone, and the badge
  says **End-to-end encrypted** in outline, with a hint that this server does
  not verify devices.
- **Realm setup.** The `melior-e2ee` client is in
  `docker/keycloak/melior-realm.json`: public, PKCE, `dpop.bound.access.tokens`,
  no full scope, a 12-hour access token, and `<frontend>/e2ee-callback` as its
  only redirect URI. Keycloak imports the realm file only when the realm
  does not exist yet, so an existing realm needs the client created once (copy
  the client entry from the realm file into **Clients → Import client** in the
  admin console); `docker/keycloak-allow-origin.sh` then adds further origins
  to both clients. Production runs with vouching off (`E2EE_ISSUER` empty in
  `deploy/compose.yaml`) until the client exists in the realm on
  `auth.faber.zip`: Keycloak shows an error page for an unknown client instead
  of redirecting back. The round trip is a top-level navigation, so Keycloak's own
  cookie is first-party: it works with third-party cookies blocked and with
  Keycloak on another site than the frontend.

What this does not protect against, and the plan to close it, is in
[`docs/e2ee-risks.md`](docs/e2ee-risks.md).

## Configuration

### Server (`melior-sfu`)

| Variable | Default | Description |
| --- | --- | --- |
| `DATABASE_URL` | required | Communities and chat history, `postgres://user:password@host:port/db`. Migrations run on first use |
| `MELIOR_BIND` | `0.0.0.0:5000` | Listen address |
| `MELIOR_INSTANCE_ID` | `melior-1` | Names this instance; must differ between instances sharing one Redis |
| `FRONTEND_URL` | `http://localhost:3000` | The origin browsers use for the app |
| `REDIS_URL` | `redis://127.0.0.1:6379` | Sessions, room leases, community events and the voice roster, `redis://[:password@]host:port[/db]` |
| `IDP_ISSUER` | `http://localhost:8300/realms/melior` | OIDC issuer; endpoints and keys are discovered from it |
| `IDP_CLIENT_ID` | `melior` | Public client (PKCE, no secret) |
| `IDP_SCOPE` | `openid email profile` | Must include `openid`; `offline_access` is refused |
| `SESSION_COOKIE_NAME` | `melior_session_id` | |
| `SESSION_TTL_SECONDS` | `3600` | Session lifetime without a token refresh |
| `TOKEN_REFRESH_LEAD_SECONDS` | `30` | Refresh tokens this long before they expire |
| `SESSION_RECHECK_SECONDS` | `15` | How often an open connection re-checks its session, its channel or community access and the room lease, and renews its voice roster entry. A room lease and a roster entry last three times this |
| `MELIOR_TURN_URLS` | unset | Comma-separated ICE server URLs handed to browsers. The server itself connects directly |
| `MELIOR_TURN_SECRET` | unset | Secret shared with the TURN server; per-user credentials are derived from it |
| `MELIOR_MESH_MAX_PARTICIPANTS` | `4` | The largest room that sends media browser to browser, and the largest `peer_to_peer` channel. `0` turns mesh off; otherwise `2` to `8`, anything else fails startup. Effectively `0` when `MELIOR_TURN_URLS` is unset: mesh needs TURN as its fallback, and the server logs that once at startup |
| `MELIOR_TURN_TTL_SECONDS` | `3600` | Lifetime of a TURN credential |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | unset | OTLP gRPC endpoint for traces, e.g. `http://127.0.0.1:4317` |
| `S3_ENDPOINT` | unset | S3-compatible storage for chat images, `http(s)://host[:port]` (path-style requests). With `S3_ACCESS_KEY` and `S3_SECRET_KEY`: all three or none, anything else fails startup. Unset: uploads answer 503 |
| `S3_ACCESS_KEY`, `S3_SECRET_KEY` | unset | The key pair requests are signed with (AWS Signature Version 4) |
| `S3_BUCKET` | `melior-images` | 3 to 63 characters of `a-z`, `0-9`, `-`, `.`; created on the first upload |
| `S3_REGION` | `us-east-1` | The region requests are signed for |
| `UNATTACHED_IMAGE_TTL_SECONDS` | `86400` | How long an uploaded image may wait for a message to carry it before it is removed; at least `60`. The sweep runs every 10 minutes, or as often as this when it is shorter |
| `KLIPY_APP_KEY` | unset | KLIPY app key handed to signed-in browsers for GIF search. Unset: GIFs are off |
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
| `E2EE_ISSUER` | empty | Frontend image only: the realm that vouches for device keys, as browsers reach it (`https://auth.example.org/realms/melior`). Served as `/e2ee.json`; empty turns vouching off. The dev server serves `apps/web/public/e2ee.json` (`http://localhost:8300/realms/melior`) instead |
| `E2EE_CLIENT_ID` | `melior-e2ee` | Frontend image only: the realm's client for DPoP-bound device tokens |

HTTPS is used only if both files exist. To use the app from another device you
need HTTPS (for example a certificate from `mkcert`), a matching
`FRONTEND_URL`, that origin added to the Keycloak client's redirect URIs, and
TURN reachable from that device (below).

### Docker Compose (`docker/.env`)

Run `docker/init-env.sh` once. It creates `docker/.env` (mode 600) from
`.env.example` with random values for `POSTGRES_PASSWORD`,
`KEYCLOAK_DB_PASSWORD`, `KEYCLOAK_ADMIN_PASSWORD`, `TURN_SECRET` and
`RUSTFS_SECRET_KEY`, and refuses to overwrite an existing file unless you pass
`--force`. Compose has no defaults for these five and will not start without
them. An older `docker/.env` lacks `RUSTFS_SECRET_KEY`: append one, for example
`echo "RUSTFS_SECRET_KEY=$(openssl rand -hex 24)" >> docker/.env`. RustFS serves
the S3 API on `127.0.0.1:9000` and its console on `127.0.0.1:9001`
(`RUSTFS_ACCESS_KEY` / `RUSTFS_SECRET_KEY`). Postgres reads its
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
Direct links between browsers use the same per-user credentials as the SFU
connection. Mesh needs TURN as its fallback, so it is off while
`MELIOR_TURN_URLS` is unset.
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
- To see why a call does not go direct, add `?debug=true` to a room URL: a panel
  shows the ICE server URLs (never the TURN username or credential), the SFU
  connection and every direct link (states, the candidate pair in use, the
  candidates gathered by type, bytes and bitrate), each link's route as the
  client can infer it, and a log of the mesh signals and state changes. Its
  Copy button puts all of it on the clipboard as JSON, without session ids.
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
- Sessions (Redis) and communities (Postgres) are shared, so a user stays
  logged in whichever instance answers. Community events go over Redis
  pub/sub, and the voice roster lives in Redis, so every instance's feeds see
  every instance's voice channels.

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
the package settings (Package settings, Danger Zone, Change visibility) so anyone
can pull them without credentials. The deploy logs in with the workflow's token,
so it does not depend on this.

A stable release then deploys itself; see [Deployment](#deployment).

## Deployment

Melior runs on the VM `melior`. The work is split between two repositories:

| This repository | `Jonohas/infra` |
|---|---|
| `deploy/compose.yaml`: the server, the frontend, Postgres, Redis and RustFS, from the release images | The VM with Docker, on the tailnet as `tag:melior-host` with Tailscale SSH, and the user `deploy` |
| `deploy/deploy.sh`: pulls one release and starts it | `/opt/melior/.env`: `FRONTEND_URL`, the issuer and client, the TURN URLs and secret, `POSTGRES_PASSWORD`, `RUSTFS_SECRET_KEY`, and optionally `KLIPY_APP_KEY` |
| `.github/workflows/deploy.yml`: runs the two on the VM | The realm `melior` on the central Keycloak at `https://auth.faber.zip`, coturn at `turn.faber.zip`, and Traefik serving `https://meet.faber.zip` |

The infrastructure repository rewrites `/opt/melior/.env` on every run, so the
app's own settings go in `deploy/compose.yaml`.

`release.yml` starts `deploy.yml` on `main` for every stable release. To deploy a
pre-release or to roll back, open **Actions**, then **deploy**, then **Run
workflow**, and enter the version (`X.Y.Z`, no `v`). The workflow:

1. Checks that the version has a GitHub Release.
2. Joins the tailnet as `tag:melior-deployer` through a federated identity, so
   no secret is stored. Only runs on `main` get in.
3. Copies `deploy/compose.yaml` and `deploy/deploy.sh` to `/opt/melior` as
   `deploy@melior`, over Tailscale SSH.
4. Runs `deploy.sh X.Y.Z`. It writes `version.env`, pulls the images and starts
   them, and it waits until every container is running. One deploy runs at a time.
5. Waits until `<FRONTEND_URL>/api/health` answers 200 through Traefik.

To read the logs on the VM:

```sh
ssh deploy@melior
cd /opt/melior && docker compose --env-file .env --env-file version.env -f compose.yaml logs -f melior-server
```

### Set up once

1. In the Tailscale admin console, under **Settings**, then **Trust credentials**,
   add an **OpenID Connect** credential:
   - Issuer: **GitHub Actions**
   - Subject: `repo:Jonohas/melior:ref:refs/heads/main`
   - Custom claim: `job_workflow_ref` =
     `Jonohas/melior/.github/workflows/deploy.yml@refs/heads/main`
   - Scope: **Auth Keys** write, with the tag `tag:melior-deployer`
2. Store its client ID and audience in this repository:

   ```sh
   gh variable set TS_RUNNER_CLIENT_ID --body '<client id>'
   gh variable set TS_RUNNER_AUDIENCE --body '<audience>'
   ```

## Tests and checks

```sh
cd melior-server
cargo test --workspace --locked      # unit tests per crate; HTTP tests for login and communities; feed and forced-close
                                     # tests; end-to-end tests with real WebRTC clients. Needs no
                                     # Keycloak, Redis, Postgres or TURN: fakes sit behind the ports.
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items --locked
cargo xtask layers                   # the dependency rules between the crates

cd melior-frontend
bun run test:unit && bun run check-types && bun run biome-check && bun run build
```

CI runs the same commands (`.github/workflows/checks.yml`). The OIDC, TURN,
session and lease adapters are exercised only by running the stack; see
`AGENTS.md` section 5. The Postgres and Redis adapters for communities and chat
have ignored tests that need a database and a Redis (use a database of its own:
the migrations run on it):

```sh
MELIOR_TEST_DATABASE_URL=postgres://user:password@127.0.0.1:5432/db \
    cargo test -p melior-infrastructure --test postgres_chat --test postgres_communities -- --ignored
MELIOR_TEST_REDIS_URL=redis://127.0.0.1:6379/9 \
    cargo test -p melior-infrastructure --test redis_communities -- --ignored
MELIOR_TEST_S3_ENDPOINT=http://127.0.0.1:9000 MELIOR_TEST_S3_ACCESS_KEY=melior \
MELIOR_TEST_S3_SECRET_KEY="$RUSTFS_SECRET_KEY" \
    cargo test -p melior-infrastructure --test s3_images -- --ignored
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
- A channel's changed media mode or limit applies to its room at the next join;
  participants already in a room over a new limit stay.
- The full check at the WebSocket upgrade and the admission probe can race with
  another join; the join itself then refuses with an `error`.
- Feed events published while an instance's Redis subscription reconnects are
  lost; the next snapshot and `structure_changed` put the client right.
- The server needs host networking in Docker, so the `app` profile works on
  Linux only.
- In a mesh room the participants see each other's IP addresses, because the
  browsers connect directly (unless a browser forces relay candidates).
- A removed participant's direct links close when the other browsers act on
  `link_close` or `peer_left`, not at the instant of removal.
- After a switch, video quality ramps up for a few seconds on the new path.
- Switching is per pair of participants. A failed direct link moves the whole
  room to the SFU until the next join or leave.
- There is no recording. A receiver on a slow link is moved to a smaller layer
  only after it has lost packets for a few seconds, so it sees a short glitch
  first.
- With a camera quality of 360p, Chrome stops sending the full simulcast layer,
  so receivers through the SFU get the half layer (320 pixels wide instead of
  640). Direct links are not affected.
