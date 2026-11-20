# Melior in diagrams

How the pieces fit together, from the outside in. The text in `README.md` is the
reference; these diagrams are the map.

1. [The system](#1-the-system)
2. [Where media flows](#2-where-media-flows)
3. [Server crates and layers](#3-server-crates-and-layers)
4. [Ports and adapters](#4-ports-and-adapters)
5. [Frontend structure](#5-frontend-structure)
6. [Logging in](#6-logging-in)
7. [Joining a room](#7-joining-a-room)
8. [Negotiation states](#8-negotiation-states)
9. [Forwarding and reusing media sections](#9-forwarding-and-reusing-media-sections)
10. [Who may enter a voice channel](#10-who-may-enter-a-voice-channel)
11. [When the server closes a connection](#11-when-the-server-closes-a-connection)
12. [Several server instances](#12-several-server-instances)
13. [Voice presence in the community](#13-voice-presence-in-the-community)
14. [Access over Tailscale](#14-access-over-tailscale)
15. [Route of a link](#15-route-of-a-link)
16. [Switching from forwarded to direct](#16-switching-from-forwarded-to-direct)

## 1. The system

The browser talks to one origin. The frontend's proxy (Vite in development,
nginx in the image) passes the backend paths on to the SFU, which is also the
backend-for-frontend for login.

```mermaid
flowchart LR
    browser["Browser<br/>React app"]

    subgraph frontend["melior-frontend"]
        proxy["Vite dev server or nginx<br/>serves the app, proxies the backend paths"]
    end

    subgraph server["melior-server"]
        sfu["melior-sfu<br/>signalling, login, community API,<br/>community feed, media forwarding"]
    end

    keycloak["Keycloak<br/>identity provider"]
    redis[("Redis<br/>sessions, refresh claims, room leases,<br/>community events pub/sub, voice roster")]
    postgres[("Postgres<br/>communities, channels, chat history,<br/>Keycloak data")]
    coturn["coturn<br/>TURN relay"]
    jaeger["Jaeger<br/>traces"]

    browser -- "HTTPS: page, /api, /login, /signin, /signout" --> proxy
    browser -- "WebSocket: /ws, /ws/communities/{id}" --> proxy
    proxy --> sfu
    browser -- "login page" --> keycloak
    browser == "media, UDP" ==> sfu
    browser -. "media, if direct fails" .-> coturn
    coturn -. "relayed media" .-> sfu

    sfu -- "OIDC: discovery, token, logout" --> keycloak
    sfu --> redis
    sfu --> postgres
    sfu -- "OTLP" --> jaeger
    keycloak --> postgres
```

## 2. Where media flows

A room sends media over one of two paths. In a room of up to
`MELIOR_MESH_MAX_PARTICIPANTS` people (default 4) the browsers connect to each
other, *direct*. In a bigger room each browser has one connection to the SFU:
it sends its own camera and microphone there and receives everyone else's from
there, *forwarded*. The SFU forwards packets; it does not decode or mix them.
TURN is the fallback for both: for a browser that cannot reach the SFU, and for
a pair of browsers that cannot reach each other. The SFU itself never uses it.

The SFU connection stays open in a mesh room, so the room can switch to
forwarding without a new connection.

```mermaid
flowchart LR
    subgraph mesh["Room of 2 to 4, direct"]
        a1["Alice's browser"]
        b1["Bob's browser"]
        c1["Carol's browser<br/>UDP to Bob blocked"]
        t1["coturn"]
        a1 <==> b1
        a1 <==> c1
        b1 -. "relayed" .-> t1
        t1 -. "relayed" .-> c1
    end

    subgraph big["Room of 5 or more, forwarded"]
        a2["Alice's browser"]
        b2["Bob's browser"]
        d2["Dave's browser<br/>UDP to the SFU blocked"]
        t2["coturn"]
        sfu["SFU"]
        a2 == "her tracks" ==> sfu
        sfu == "everyone else's tracks" ==> a2
        b2 == "his tracks" ==> sfu
        sfu == "everyone else's tracks" ==> b2
        d2 -. "his tracks" .-> t2
        t2 -. "relayed" .-> sfu
        sfu -. "everyone else's tracks" .-> t2
        t2 -. "relayed" .-> d2
    end
```

In a mesh room with `n` participants each browser sends `n - 1` copies of its
tracks (camera, microphone and, while it shares, its screen in a fixed third
section of each link), and the SFU sends nothing; its connection is open but the uplink is
paused. In a forwarded room the SFU receives `n` sets of tracks and sends
`n * (n - 1)`. Signalling always goes through the server, which relays the
offers, answers and candidates of the direct links.

During a switch both paths carry media, one pair at a time; see
[section 15](#15-route-of-a-link) and [section 16](#16-switching-from-forwarded-to-direct).

## 3. Server crates and layers

One crate per layer. An arrow means "depends on"; `cargo xtask layers` fails the
build if an arrow points any other way, or if a crate uses a library its layer
may not.

```mermaid
flowchart TB
    sfu["melior-sfu, L3<br/>binary: composition root, integration tests"]
    infra["melior-infrastructure, L2<br/>driven adapters: webrtc-rs, OIDC, Redis,<br/>Postgres, TURN credentials, telemetry, config"]
    interfaces["melior-interfaces, L2<br/>driving adapters: HTTP endpoints, WebSocket"]
    app["melior-application, L1<br/>use cases and ports"]
    domain["melior-domain, L0<br/>rooms, negotiation, communities and<br/>permissions, users<br/>pure and synchronous"]

    sfu --> infra
    sfu --> interfaces
    sfu --> app
    infra --> app
    infra --> domain
    interfaces --> app
    interfaces --> domain
    app --> domain
```

| Crate | May not use |
| --- | --- |
| `melior-domain` | any async runtime, logging, or I/O library |
| `melior-application` | axum, webrtc, redis, sqlx, reqwest, opentelemetry |
| `melior-infrastructure` | a web framework; `melior-interfaces` |
| `melior-interfaces` | webrtc, redis, sqlx, reqwest; `melior-infrastructure` |

## 4. Ports and adapters

The use cases name traits, never technologies. Driving adapters call the
services; driven adapters implement the ports. Tests put in-memory
implementations and fakes behind the same ports.

```mermaid
flowchart LR
    subgraph driving["melior-interfaces"]
        http["HTTP handlers<br/>/login /signin /signout /api/identity/me<br/>/api/communities /api/channels /api/invites"]
        ws["WebSocket handler<br/>/ws"]
        feed["Community feed<br/>/ws/communities/{id}"]
    end

    subgraph application["melior-application"]
        auth["AuthService"]
        comm["CommunityService"]
        chat["ChatService"]
        conf["ConferenceService"]
        queue["PresenceQueue"]
    end

    subgraph ports["ports, traits"]
        idp(["IdentityProvider"])
        store(["SessionStore"])
        repo(["CommunityRepository"])
        events(["CommunityEvents"])
        roster(["VoiceRoster"])
        chatrepo(["ChatRepository"])
        media(["MediaEngine, MediaSession"])
        ice(["IceServerProvider"])
        leases(["RoomLeases"])
        presence(["PresenceSink"])
        sink(["SignalSink"])
    end

    subgraph driven["melior-infrastructure"]
        oidc["OidcIdentityProvider"]
        rsess["RedisSessionStore"]
        pg["PostgresCommunityRepository"]
        revents["RedisCommunityEvents"]
        rroster["RedisVoiceRoster"]
        pgchat["PostgresChatRepository"]
        rtc["WebRtcMediaEngine"]
        turn["TurnRestCredentials"]
        rlease["RedisRoomLeases"]
    end

    http --> auth
    http --> comm
    http --> chat
    ws --> auth
    ws --> comm
    ws --> conf
    ws --> chat
    feed --> auth
    feed --> comm

    auth --> idp
    auth --> store
    comm --> repo
    comm --> events
    comm --> roster
    chat --> chatrepo
    conf --> media
    conf --> ice
    conf --> leases
    conf --> presence
    conf --> sink
    queue --> roster
    queue --> events

    oidc -. implements .-> idp
    rsess -. implements .-> store
    pg -. implements .-> repo
    revents -. implements .-> events
    rroster -. implements .-> roster
    pgchat -. implements .-> chatrepo
    rtc -. implements .-> media
    turn -. implements .-> ice
    rlease -. implements .-> leases
    queue -. implements .-> presence
    ws -. implements .-> sink
```

`WebRtcMediaEngine` reports what it observes (a track published, a track ended,
the connection established, a local ICE candidate) back to `ConferenceService`
through the `MediaEvents` port. `ConferenceService` reports who is in which
voice channel to the `PresenceSink` port without waiting; `PresenceQueue`
implements it in the application and keeps the voice roster and the community
events from one task.

## 5. Frontend structure

Routes compose features; inside a feature, dependencies run one way.

```mermaid
flowchart TB
    subgraph routes["routes, file-based"]
        root["__root"]
        gate["_authenticated<br/>loads the session, redirects to /login,<br/>shows the server rail"]
        home["/ : home"]
        community["/c/$communityId<br/>channel sidebar, community feed"]
        channel["/c/$communityId/$channelId<br/>text or voice channel"]
        invite["/invite/$code"]
        error["/error"]
    end

    subgraph lib["lib"]
        authlib["auth<br/>login gate, session loader"]
        query["query client and keys"]
    end

    subgraph communities["features/communities"]
        mcomp["components"] --> mhooks["hooks<br/>queries, mutations, feed connection"]
        mhooks --> mapi["api<br/>communities, channels, roles, invites, feed"]
        mapi --> mdomain["domain<br/>community, permissions, channel tree,<br/>feed messages, voice roster"]
    end

    subgraph conference["features/conference"]
        ccomp["components"] --> chooks["hooks<br/>use-conference"]
        chooks --> capi["api<br/>signalling client, peer session, local media"]
        capi --> cdomain["domain<br/>messages and parser, state reducer,<br/>track reconciliation, reconnect policy"]
    end

    subgraph chatf["features/chat"]
        tcomp["components"] --> thooks["hooks"]
        thooks --> tapi["api"]
        tapi --> tdomain["domain"]
    end

    root --> gate
    root --> error
    gate --> home
    gate --> community
    gate --> invite
    gate --> authlib
    community --> channel
    home --> mcomp
    community --> mcomp
    channel --> mcomp
    invite --> mcomp
    mcomp --> ccomp
    mcomp --> tcomp
    mhooks --> query
    authlib --> query
```

## 6. Logging in

The server runs the whole OpenID Connect exchange. The browser only follows
redirects and ends up with an opaque session cookie; tokens stay in Redis.

```mermaid
sequenceDiagram
    autonumber
    actor user as User
    participant browser as Browser
    participant sfu as SFU, via the frontend proxy
    participant redis as Redis
    participant kc as Keycloak

    user->>browser: open /invite/Xy7Qp2Lm9a
    browser->>sfu: GET /api/identity/me
    sfu-->>browser: 401
    browser->>sfu: GET /login?redirectTo=/invite/Xy7Qp2Lm9a
    Note over sfu: make PKCE verifier and nonce
    sfu-->>browser: 302 to Keycloak, set bff_pkce cookie
    browser->>kc: authorization request with code challenge
    user->>kc: username and password
    kc-->>browser: 302 to /signin with code and state
    browser->>sfu: GET /signin, sends bff_pkce cookie
    sfu->>kc: redeem code with the verifier
    kc-->>sfu: access, refresh and id tokens
    Note over sfu: verify signatures, issuer, expiry,<br/>audience and nonce, read roles
    sfu->>redis: store session with TTL
    sfu-->>browser: 302 to /invite/Xy7Qp2Lm9a, set session cookie
    browser->>sfu: GET /api/identity/me
    sfu->>redis: load session
    sfu-->>browser: 200 subject, username, email, roles
```

Later requests refresh the tokens shortly before they expire. A refresh that
Keycloak rejects ends the session; one that cannot be attempted leaves it alone.

## 7. Joining a room

The upgrade is refused before any WebSocket exists unless every check passes:
the channel permissions of [section 10](#10-who-may-enter-a-voice-channel), the
room lease (503) and the room's capacity (409 when it is full). A browser cannot
see the status of a refused upgrade, so the client asks
`GET /api/channels/{channel_id}/admission` for it. After that the client offers
once, and the server drives every later renegotiation.

```mermaid
sequenceDiagram
    autonumber
    participant bob as Bob's browser
    participant sfu as SFU
    participant alice as Alice's browser, already in the room

    bob->>sfu: GET /ws?channel_id=standup, session cookie
    Note over sfu: origin, session and role,<br/>channel permissions, room lease,<br/>capacity, 409 when full
    sfu-->>bob: 101 Switching Protocols
    sfu->>bob: session: ids, username, ICE servers, participants
    sfu->>alice: peer_joined: Bob
    bob->>sfu: sdp_offer with Bob's camera and microphone
    sfu->>bob: sdp_answer
    bob-->>sfu: ice candidates, both directions
    Note over bob,sfu: ICE and DTLS complete, media connection is up
    sfu->>bob: sdp_offer with tracks list: Alice's sections
    bob->>sfu: sdp_answer
    Note over sfu: Bob's tracks arrive
    sfu->>alice: sdp_offer with tracks list: Bob's sections
    alice->>sfu: sdp_answer
    Note over bob,alice: both now receive the other's audio and video
```

The `tracks` list in every server offer is complete: `mid` and `session_id` for
each media section that carries someone's track. Clients attribute media from
that list, never from stream ids.

## 8. Negotiation states

One offer/answer exchange at a time per participant. A change that arrives while
an offer is outstanding is remembered and negotiated when the answer is in.

```mermaid
stateDiagram-v2
    [*] --> AwaitingClientOffer: joined
    AwaitingClientOffer --> Connecting: first client offer answered
    Connecting --> Stable: media connection established
    Stable --> AwaitingAnswer: tracks changed, server sends offer
    AwaitingAnswer --> AwaitingAnswer: tracks changed again, remembered
    AwaitingAnswer --> Stable: answer applied, nothing pending
    AwaitingAnswer --> AwaitingAnswer: answer applied, pending changes offered
    AwaitingAnswer --> Stable: offer failed or answer rejected
    Stable --> Stable: client re-offer answered
```

A participant receives other people's tracks only from `Stable` onwards, so a
renegotiation can never race the initial connection setup.

## 9. Forwarding and reusing media sections

A forwarder reads one published track and writes it to the outlet of every
subscriber. An outlet is one media section of a subscriber's connection. It
outlives the track it was opened for.

```mermaid
flowchart LR
    subgraph pub["Alice's connection"]
        cam["Alice's video<br/>RTP in"]
    end

    fwd["Forwarder<br/>strips header extensions"]

    subgraph bobc["Bob's connection"]
        ob["Outlet, mid 2<br/>rewrites sequence numbers and timestamps"]
    end

    subgraph carolc["Carol's connection"]
        oc["Outlet, mid 3<br/>rewrites sequence numbers and timestamps"]
    end

    cam --> fwd
    fwd --> ob
    fwd --> oc
    ob -. "PLI, FIR" .-> fwd
    oc -. "PLI, FIR" .-> fwd
    fwd -. "keyframe request, rate limited" .-> cam
```

When a publisher leaves, the outlets that carried their tracks go idle and take
the next publisher's track of the same codec, so a session description stops
growing with churn.

```mermaid
sequenceDiagram
    participant sfu as SFU
    participant bob as Bob's browser

    Note over sfu,bob: mid 2 carries Alice's video
    Note over sfu: Alice leaves, outlet for mid 2 goes idle
    sfu->>bob: peer_left: Alice
    sfu->>bob: sdp_offer, mid 2 inactive, tracks: empty
    bob->>sfu: sdp_answer
    Note over sfu: Dave joins and publishes video,<br/>outlet for mid 2 switches source
    sfu->>bob: sdp_offer, mid 2 sending, tracks: mid 2 is Dave
    bob->>sfu: sdp_answer
    Note over sfu,bob: same section, same SSRC, sequence numbers<br/>and timestamps continue without a jump
```

## 10. Who may enter a voice channel

The same decision is taken at the WebSocket upgrade and again on every re-check
while someone is connected; the re-check leaves out the capacity, so a lowered
limit does not remove anyone. Between the permissions and the capacity the
upgrade also needs the room lease (503, see [section 12](#12-several-server-instances)).

```mermaid
flowchart TD
    start(["User wants to join a voice channel"]) --> role{"Has the member<br/>or admin role?"}
    role -- no --> refuse["Refused, 403"]
    role -- yes --> exists{"Does the channel<br/>exist?"}
    exists -- no --> unknown["Refused, 404<br/>a channel is never created by joining it"]
    exists -- yes --> member{"Member of the<br/>channel's community?"}
    member -- no --> refuse
    member -- yes --> voice{"Voice channel?"}
    voice -- "no, text" --> refuse
    voice -- yes --> base["Base: @everyone's permissions<br/>plus those of the member's roles"]
    base --> shortcut{"Owner, or administrator<br/>in the base?"}
    shortcut -- yes --> full{"Room already<br/>at its capacity?"}
    shortcut -- no --> overrides["Overrides: the category's if synced,<br/>else the channel's own.<br/>@everyone, then the member's roles,<br/>then the member: deny, then allow"]
    overrides --> perms{"view_channel<br/>and connect?"}
    perms -- no --> refuse
    perms -- yes --> full
    full -- yes --> conflict["Refused, 409"]
    full -- no --> admit["Admitted"]
```

## 11. When the server closes a connection

Every `SESSION_RECHECK_SECONDS` each open connection asks three questions. Only
a definite "no" ends it; if a store cannot be reached, the connection stays.

```mermaid
flowchart TD
    tick(["Re-check timer fires"]) --> session{"Session still valid?<br/>refreshes tokens if due"}
    session -- "ended" --> c1["closed: session_ended<br/>client logs in again"]
    session -- "valid, or cannot tell" --> access{"Still has view_channel<br/>and connect on this channel?"}
    access -- "no" --> c2["closed: removed<br/>client stops"]
    access -- "yes, or cannot tell" --> lease{"This instance still<br/>holds the room lease?"}
    lease -- "another instance holds it" --> c3["closed: room_moved<br/>client reconnects"]
    lease -- "yes, renewed, or cannot tell" --> stay(["Connection stays open"])
```

"No" there also covers a member who was kicked or left, and a channel or
community that was deleted. On any other loss (socket drops, peer connection fails, an upgrade answered
with 503) the client reconnects by itself with exponential backoff.

## 12. Several server instances

A room's media state lives in one instance's memory, so a room is pinned: the
proxy hashes the channel id to pick an instance, and a lease in Redis guarantees
that only one instance ever hosts a given room. A community feed works on any
instance, because voice presence and community events cross instances through
Redis (see [section 13](#13-voice-presence-in-the-community)). The Vite proxy
hashes `/ws/communities/<id>` by community id to spread the feeds out; nginx
sends it round robin.

```mermaid
flowchart LR
    b1["Browsers in room A"]
    b2["Browsers in room B"]
    proxy["Proxy<br/>/ws: consistent hash of channel_id<br/>/ws/communities/{id}: any instance<br/>other paths: any instance"]
    i1["Instance melior-1<br/>hosts room A"]
    i2["Instance melior-2<br/>hosts room B"]
    redis[("Redis<br/>room-host:A = melior-1<br/>room-host:B = melior-2<br/>sessions, voice roster,<br/>melior:community:* pub/sub")]
    pg[("Postgres<br/>communities, channels, chat")]

    b1 --> proxy
    b2 --> proxy
    proxy -- "room A" --> i1
    proxy -- "room B" --> i2
    i1 -- "hold and renew lease,<br/>publish and subscribe" --> redis
    i2 -- "hold and renew lease,<br/>publish and subscribe" --> redis
    i1 --> pg
    i2 --> pg
    b1 == "media" ==> i1
    b2 == "media" ==> i2
```

When an instance stops, its rooms come back on another one after the lease
expires:

```mermaid
sequenceDiagram
    participant bob as Bob's browser
    participant proxy as Proxy
    participant one as Instance melior-1
    participant two as Instance melior-2
    participant redis as Redis

    Note over one,redis: melior-1 holds the lease for the room
    one--xbob: connection lost, melior-1 stopped
    bob->>proxy: reconnect to /ws
    proxy->>two: upgrade
    two->>redis: hold lease
    redis-->>two: held by melior-1
    two-->>bob: 503
    Note over bob: wait with backoff, try again
    Note over redis: lease expires, three re-check intervals
    bob->>proxy: reconnect to /ws
    proxy->>two: upgrade
    two->>redis: hold lease
    redis-->>two: taken
    two-->>bob: 101, session, and negotiation starts over
```

## 13. Voice presence in the community

Members who are not in a call still see who is in each voice channel, and
whether they speak, show a camera or a screen, or are muted. A room never waits
for this: it only queues a report, and one task per instance stores it and
publishes it.

```mermaid
sequenceDiagram
    autonumber
    participant bob as Bob's browser, in the call
    participant one as Instance melior-1, hosts the room
    participant redis as Redis
    participant two as Instance melior-2
    participant carol as Carol's browser, feed on melior-2

    bob->>one: voice_state: speaking true
    Note over one: room checks the claim under its lock,<br/>sends voice_state to everyone in the room
    Note over one: ConferenceService reports to the PresenceSink,<br/>PresenceQueue takes it in order
    one->>redis: put roster entry melior:voice:{community}:{session}, TTL
    one->>redis: PUBLISH melior:community:{community}, voice_updated
    redis-->>one: to the instance's subscriber
    redis-->>two: to the instance's subscriber
    Note over one,two: each subscriber fans the event out<br/>to the local feeds of that community
    Note over two: FeedFilter: Carol may view the channel
    two->>carol: voice_updated: Bob, speaking
```

A feed drops voice entries and messages for channels its member may not view.
A new feed, and every feed after a re-check, starts from a `snapshot` read from
the roster. An entry lives three re-check intervals and is renewed on every
re-check, so the participants of an instance that stopped disappear with the
next snapshot.

## 14. Access over Tailscale

`docker/tailscale.sh up` publishes the app and Keycloak on the tailnet over
HTTPS. Media does not pass through `tailscale serve`; it goes to the machine's
tailnet address directly.

```mermaid
flowchart LR
    device["Device on the tailnet"]

    subgraph host["The machine running Melior"]
        serve["tailscale serve<br/>HTTPS with Tailscale certificates"]
        vite["Frontend, port 3000"]
        kc["Keycloak, port 8300"]
        sfu["SFU<br/>port 5000 and UDP media ports"]
        coturn["coturn, port 3478<br/>on the tailnet address"]
    end

    device -- "https, port 8443" --> serve
    device -- "https, port 10000" --> serve
    serve -- "8443" --> vite
    serve -- "10000" --> kc
    vite -- "/ws, /api, login paths" --> sfu
    device == "media, UDP to the tailnet address" ==> sfu
    device -. "media, if direct fails" .-> coturn
    coturn -.-> sfu
```

## 15. Route of a link

The server keeps one link per pair of participants. Its route says which path
carries the pair's media. Every transition adds one to the link's `epoch`, and a
`route_live` report counts only for the current epoch.

```mermaid
stateDiagram-v2
    [*] --> Forwarded: second participant joins, epoch 0
    Forwarded --> ToDirect: target is direct<br/>send route direct, arm 10 s timeout
    ToDirect --> Direct: both sides reported<br/>unsubscribe both from the SFU
    ToDirect --> ToForwarded: target becomes forwarded<br/>send route forwarded
    ToDirect --> ToForwarded: timeout fires<br/>pin the room to forwarded
    Direct --> ToForwarded: target becomes forwarded<br/>subscribe both to the SFU, send route forwarded
    Direct --> ToForwarded: link_failed<br/>pin the room to forwarded, subscribe both to the SFU
    ToDirect --> ToForwarded: link_failed<br/>pin the room to forwarded
    ToForwarded --> Forwarded: both sides reported<br/>send link_close
    ToForwarded --> ToDirect: target becomes direct<br/>send route direct, arm 10 s timeout
    Forwarded --> [*]: either participant leaves
    ToDirect --> [*]: either participant leaves
    Direct --> [*]: either participant leaves
    ToForwarded --> [*]: either participant leaves
```

`ToForwarded` has no timeout: the direct link keeps working until both sides
report. A `link_failed` from either side acts like the timeout, in `ToDirect`,
`Direct` and `ToForwarded`; in `ToForwarded` the link already moves back, so only
the pin changes. In `Forwarded` it is ignored. On a leave, the remaining participant gets `link_close` if the link was
in any state but `Forwarded`.

## 16. Switching from forwarded to direct

Alice's session id sorts lower than Bob's, so she makes the offer on the direct
link. Both browsers keep playing the SFU path until the direct path has decoded
media. Bob joins a room where Alice is alone and the target is `direct`.

```mermaid
sequenceDiagram
    autonumber
    participant alice as Alice's browser
    participant sfu as SFU, melior-server
    participant bob as Bob's browser

    Note over alice,bob: link Forwarded, epoch 0, media through the SFU
    sfu->>alice: route: Bob, direct, epoch 1
    sfu->>bob: route: Alice, direct, epoch 1
    Note over sfu: link ToDirect, arm 10 s timeout
    alice->>sfu: peer_offer to Bob
    sfu->>bob: peer_offer from Alice
    bob->>sfu: peer_answer to Alice
    sfu->>alice: peer_answer from Bob
    alice-->>bob: peer_ice, both directions, relayed
    Note over alice,bob: direct media flows, the SFU path still plays
    Note over alice: direct video decodes, show it
    alice->>sfu: route_live: Bob, epoch 1
    Note over bob: direct video decodes, show it
    bob->>sfu: route_live: Alice, epoch 1
    Note over sfu: both reported, link Direct
    sfu->>alice: sdp_offer without Bob's tracks
    alice->>sfu: sdp_answer
    sfu->>bob: sdp_offer without Alice's tracks
    bob->>sfu: sdp_answer
    sfu->>alice: uplink: active false
    sfu->>bob: uplink: active false
    Note over alice,bob: media only over the direct link, no tile froze
```

The `uplink` message goes to a participant only when every one of its links is
`Direct`. Going back is the same sequence in the other direction: `route`
`forwarded` first, the SFU subscriptions return and `uplink` `active` is
`true`, and `link_close` follows once both report.
