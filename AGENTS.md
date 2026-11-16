# AGENTS.md: guide for AI agents and contributors

Melior is a video-conferencing system: a Rust SFU that also runs the login
(`melior-server/`) and a React client (`melior-frontend/`), with the
infrastructure in `docker/`. Read this file first, then `README.md`.

## 1. Orientation

| Read | Why |
|---|---|
| `README.md` | What the system does, how to run it, configuration, the login flow and the signalling protocol |
| `melior-server/crates/<name>/src/lib.rs` | The crate doc comment states what the crate owns and its invariants |
| `melior-server/xtask/src/layers.rs` | The dependency rules, as data |
| `melior-frontend/README.md` | Frontend commands, routes and layering |

## 2. Workspace map

```text
melior-server/                 Cargo workspace
  crates/
    melior-domain          L0  rooms, participants, negotiation rules, room access, users. Pure and synchronous
    melior-application     L1  use cases (ConferenceService, AuthService, RoomAccessService) and their ports
    melior-infrastructure  L2  driven adapters: webrtc-rs, OIDC, Redis, Postgres, TURN credentials, telemetry, env config
    melior-interfaces      L2  driving adapters: login and room endpoints, signalling WebSocket
    melior-sfu             L3  the binary: composition root, plus the integration tests
  xtask/                       cargo xtask layers
melior-frontend/               Bun workspace + Turborepo
  apps/web                     TanStack Start on Vite; routes/, lib/, features/<feature>/{domain,api,hooks,components}
docker/                        compose: Postgres, Redis, Keycloak, coturn, Jaeger, and the app profile
```

**Layering is enforced** by `cargo xtask layers` (rules in `xtask/src/layers.rs`, run in CI):

- A crate depends only on workspace crates of a lower layer. The two adapter
  crates share a layer, so neither can depend on the other.
- `melior-domain` uses no async runtime, no logging and no I/O library.
- `melior-application` uses no adapter library (axum, webrtc, redis, sqlx, reqwest, opentelemetry, ...).
- `melior-infrastructure` uses no web framework; `melior-interfaces` uses no
  media stack, storage client or HTTP client.
- A new crate must get a row in `TABLE` in `xtask/src/layers.rs`.

The frontend has the same rule in a different form: `components -> hooks -> api
-> domain` inside a feature, imports only through `@/features/<feature>/<layer>`
barrels, and Biome's `noRestrictedImports` rejects deep imports.

## 3. Golden rules

### Never crash (outranks feature work)

The server holds every live conference in memory. A panic in one request ends
all of them. A malformed message, an invalid token, an unreachable Keycloak or
Redis, or a peer that disappears mid-negotiation must produce an error the
caller can act on, never a panic.

- **Non-test code never panics.** No `unwrap()`, `expect()`, `panic!`,
  `unreachable!`, `todo!` or `unimplemented!`. Return the crate's error type and
  propagate with `?`. Use `unwrap_or*` only where the fallback is truly correct.
- **Enforced by clippy.** Every crate starts with
  `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]`.
  `clippy.toml` allows them in tests.
- **No `unsafe`.** The workspace sets `unsafe_code = "forbid"`.
- **Everything from the network is hostile.** Client messages, cookies, query
  strings, `state` and `redirectTo`, tokens and identity provider responses are
  parsed into domain types at the edge and rejected there if they do not fit.
  Nothing is trusted because of where it claims to come from.
- **Don't cascade.** Handle lock poisoning with
  `lock().unwrap_or_else(PoisonError::into_inner)`. A failing subscriber must
  not stop a track for the others; a failing client must not block a room.
- **Prove it.** Every crash or security fix comes with a test that failed before the fix.

### Rules

1. **Decisions live in the domain.** A rule about rooms, negotiation, roles or
   sessions is a method on a domain type with unit tests, and it returns what
   has to happen; the application carries it out through ports. Adapters and
   handlers only translate.
2. **Ports, not adapters.** The application names no concrete technology. A new
   external dependency (a queue, a database) is a trait in a `ports` module plus
   an adapter in `melior-infrastructure`, with an in-memory implementation or
   fake for tests. Adapters connect lazily: the server must start while its
   dependencies are still coming up.
3. **Tests are the gate.** Every change comes with tests at the layer where the
   decision is: domain unit tests, application tests against fake ports, HTTP
   tests against a real server with a fake identity provider, and the
   end-to-end test with real WebRTC clients when media paths change.
4. **Assume several instances.** Anything that must be true across server
   instances goes through shared storage: sessions and token-refresh claims in
   Redis, the room registry in Postgres, and the per-room lease that keeps a
   room on one instance. In-process locks only order work inside one instance.
   Only a definite answer may end a user's connection: when a store cannot be
   asked, the connection stays.
5. **One room, one lock.** Every change to a room and the media operations that
   follow from it run under that room's lock. Nothing awaited under it may wait
   on a client: signals are queued, answers arrive as separate use cases. Never
   hold a `std::sync` guard across an `.await`.
6. **Tokens stay on the server.** The browser gets an opaque session cookie
   only. Session ids, tokens, cookies and credentials never appear in logs or
   error messages (`AuthSessionId`'s `Debug` is redacted for this reason).
   TURN credentials are per user and expire.
7. **Redirects stay inside the application.** Every post-login redirect goes
   through `ReturnPath`; every absolute URL is built from `FRONTEND_URL`, never
   from request headers.
8. **The protocol has two ends.** A change to a signalling message or to a login
   endpoint changes `melior-interfaces` (DTOs and their tests), the frontend's
   `features/conference/domain` parser and its tests, and the tables in
   `README.md`, in the same change.
9. **Verify in a browser when behaviour changes.** Unit tests do not prove that
   media flows or that a login completes. Run the stack (section 5) and check
   it; say so when you could not.
10. **Configuration is validated at startup** in
   `melior-infrastructure::config`, with a test per rule, and documented in the
   tables in `README.md`. Secrets are generated into the git-ignored
   `docker/.env` by `docker/init-env.sh`; never commit one, never give one a
   default, and keep them out of logs and `Debug` output (`Config`'s is written
   by hand for this).
11. **Third-party code.** Learn from other projects' behaviour; do not copy code
    without a compatible licence and an attribution.

## 4. Before you finish a task

Server, from `melior-server/`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items --locked
cargo xtask layers
```

Frontend, from `melior-frontend/`:

```sh
bun run biome-check
bun run check-types
bun run test:unit
bun run build
```

CI (`.github/workflows/checks.yml`) runs exactly these. Keep the tree building
at every step.

### Commits and pull requests

**No AI attribution, anywhere.** A commit message, a pull request title or
description, a code comment or a document never says that an AI assistant wrote
or helped write it: no `Co-Authored-By:` trailer naming an assistant, no
"Generated with ..." line, no badge or emoji to that effect. The author of a
commit is the person who commits it. This is a project decision, it holds for
every tool and every agent, and a tool's default behaviour does not override
it. A commit that carries such a line is to be rewritten before it is pushed.

Write a commit message for someone reading the history later: a subject that
says what changed, and a body that says why when the subject cannot.

## 5. Running the whole system

```sh
cd docker && ./init-env.sh && docker compose up -d   # Postgres, Redis, Keycloak, coturn, Jaeger
cd melior-server && set -a && . ../docker/.env && set +a && \
  DATABASE_URL="postgres://$POSTGRES_USER:$POSTGRES_PASSWORD@127.0.0.1:5432/$POSTGRES_DB" \
  MELIOR_TURN_URLS='turn:127.0.0.1:3478?transport=udp' MELIOR_TURN_SECRET="$TURN_SECRET" \
  OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4317 cargo run
cd melior-frontend && bun dev                        # http://localhost:3000
```

Test users (password = username): `alice` (admin), `bob`, `carol` (members),
`guest` (no role, must be refused). Use `localhost`, not `127.0.0.1`.

What only a run of the stack proves, and how to check it:

- **Media and login**: headless Chromium with `--use-fake-device-for-media-stream
  --use-fake-ui-for-media-stream` and one browser context per user; each page
  must show the others' videos playing (`videoWidth > 0`, `currentTime` rising).
- **TURN**: add `?relay=true` to the room URL; coturn's log must show
  `ALLOCATE processed, success` for usernames of the form `<expiry>:<user id>`.
- **Section reuse**: let a user leave and rejoin a few times; the others'
  videos must keep playing.
- **Access and sessions**: restrict the room with `PUT /api/rooms/<id>` and
  check the uninvited user is told `removed`; end a session and check its
  socket closes. Set `SESSION_RECHECK_SECONDS=3` to keep this short.
- **Several instances**: two servers with different `MELIOR_BIND` and
  `MELIOR_INSTANCE_ID`, `MELIOR_SERVER_URLS` on the frontend; a room must be
  refused (503) by the instance that does not host it and taken over after the
  host stops.
- **Traces**: `http://localhost:16686/api/services` lists `melior-sfu`.

## 6. Parallel agents

- Use your own target dir (`CARGO_TARGET_DIR=target/agent-<name>`) and edit only
  the files you own.
- Keep every `Cargo.toml` valid at all times: the `crates/*` glob means one
  broken manifest breaks every build.
- If another agent's in-progress edit breaks the build, wait and retry. Do not
  fix their files.
- Use ports other than 3000, 5000, 5001 and 9333 for anything you start, and
  stop it when you are done. Do not restart or remove the compose containers
  while someone else is testing against them.

## 7. Toolchains

- Rust is pinned in `melior-server/rust-toolchain.toml`; the `FROM rust:` line in
  `melior-server/Dockerfile` carries the same version. Bumping it is a change of
  its own.
- Bun is pinned by `packageManager` in `melior-frontend/package.json`;
  dependency versions live in the root `catalogs`.
- Lockfiles are committed and every command uses them (`--locked`,
  `--frozen-lockfile`).
