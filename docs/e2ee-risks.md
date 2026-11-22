# End-to-end encryption: risks and pitfalls

What the browser implementation of end-to-end encryption (built in November
2026, see `README.md`, End-to-end encryption) protects, what it does not, and
what tends to go wrong. Read it before you change the key exchange, the frame
format or the relay, and before you tell anyone the server cannot see a call.

## Who can read a call

| Attacker | Can read media? | Why |
|---|---|---|
| The network, a Wi-Fi snooper, an ISP | No | DTLS-SRTP per hop, and the frames inside are encrypted again end to end. |
| TURN (coturn) | No | It relays SRTP packets whose frames it cannot decrypt. |
| The SFU process, its logs, a memory dump of it | No | It forwards frames encrypted with keys it never receives. |
| Postgres or Redis | No | No media key, device key or call key is ever stored on the server. |
| Someone who left the call | Not what is said after they left | Every leave rotates every sender's media key. |
| Someone who joins | Not what was said before they joined | Every join rotates every sender's media key. |
| An operator or attacker who controls the Melior server and **swaps public keys** | No | The realm vouches for every device key; a swapped key gets no media key. See risk 1. |
| An operator or attacker who controls **Keycloak** | **Yes** | The realm could vouch for a key of its own. See risk 1. |
| An operator or attacker who controls **what JavaScript the server delivers** | **Yes** | See risk 2. |
| Malware or a malicious extension in a participant's browser | Yes | Out of scope for any end-to-end scheme. |

## Risks that remain

### 1. Whoever controls the realm can vouch for a fake key

The Melior server relays the `hello` messages that carry each participant's
device and call keys, so on its own it could answer with its own keys for both
sides and sit in the middle. Keycloak closes that: each hello carries a token
of the realm's `melior-e2ee` client, bound to the device key with DPoP, and
peers send their media key only to a device the realm vouches for under the
username the call shows (`README.md`, End-to-end encryption, Vouching). The
hello signature binds the call key to the device key and to both session ids,
so a captured hello cannot be replayed into another call or to another
participant.

What remains:

- **Keycloak is the trust anchor.** Whoever controls the realm (its admin, its
  signing keys, its database) can issue a token for a key of their own and
  read calls. The protection is only as separate as Keycloak is from the
  Melior server: other host, other credentials, other people with access.
- **A login is enough.** Anyone who can log in as a user (a phished password,
  a session cookie of Keycloak) can get a token for their own device as that
  user. That is the same power as joining the call as them.
- **No Keycloak session, no token.** The token comes from a `prompt=none`
  round trip to the realm before a call. If the person's Keycloak session has
  ended (but Melior's has not), the realm answers `login_required`, the device
  counts as unverified and receives nothing until the next attempt 10 minutes
  later or a fresh login.
- **A token lasts 12 hours.** A device whose key was stolen within that time
  can still be vouched for until the token expires. Revoking a user in
  Keycloak does not revoke a token already issued.
- **Without `E2EE_ISSUER` there is no vouching.** The frontend then sends keys
  to every device and the badge says every device is unverified: a server
  that swaps keys is not caught.

How to strengthen it later: append every token the realm issues to an
append-only log that an independent witness signs, and let each browser check
that only its own devices appear under its username (key transparency). It
detects a realm that misbehaves. **Not built.**

### 2. The server delivers the code that holds the keys

The frontend is served from the same origin as the server. Whoever controls
that origin can ship JavaScript that reads frames before encryption or sends
the media keys away. Non-extractable `CryptoKey`s stop the private keys from
being exported; they do not stop malicious code from using them or from
reading the video. No browser-only design closes this.

What narrows it: hosting the static frontend apart from the SFU (other host,
other credentials, Subresource Integrity on every chunk, reproducible builds
with published hashes). What closes it: a signed desktop app (Electron) whose
updates are checked against a release key that no server holds. Both were
discussed and deferred; the app stays in the browser for now.

### 3. Metadata is not encrypted

The server sees who is in which call, when, for how long, who speaks (audio
level and voice state), frame sizes and timing, and the clear prefix of each
frame (the VP8 header with the resolution, the Opus TOC byte). Chat goes
through the server and is stored in Postgres in the clear.

## Pitfalls met while building it

These broke the implementation once. Each now has a fix and a test.

- **ICE bursts starved the key exchange.** A browser on a host with many
  network interfaces sends dozens of ICE candidates per direct link. They used
  the same relay rate limit as key messages, the server dropped 913 messages in
  one three-person test, and one participant's media stayed undecryptable. Key
  messages now have a bucket of their own
  (`melior-interfaces/src/ws/limiter.rs`, `RelayLimits`).
- **Key messages that arrive while the call's keys are being created.** The
  `session` handler awaits WebCrypto and IndexedDB; a peer's hello that came in
  meanwhile was dropped. Such work is now queued (`withE2ee` in
  `hooks/session-messages.ts`).
- **A lost key message is permanent unless someone asks again.** A browser now
  says hello again every 3 seconds, up to 5 times, until a peer's first key
  arrives; a hello makes the peer send its keys again.
- **WebCrypto completes off the JavaScript thread.** Tests that fake timers
  must not fake `setImmediate`, and must wait in real time for crypto to
  finish (`e2ee-session.test.ts`, `flush`).
- **`receiver.transform = null` is not a proof test.** Removing a receiver's
  transform mid-call did not show garbled video in Chromium 158. The proof that
  frames are encrypted is that every received frame passes AES-GCM
  authentication in the worker; an unencrypted frame is dropped there.

- **A hidden frame does not get Keycloak's cookie.** The first build fetched
  the token with a `prompt=none` login in a hidden iframe. Browsers that block
  third-party cookies (Safari by default, Firefox strict mode, Chromium with
  the setting on) send no Keycloak cookie into a frame of another site, so
  those devices got no token and saw nothing. The round trip is now a
  full-page redirect before the call starts.

- **RED audio arrives whole.** With `audio/red` negotiated, Chromium hands
  the encoded transform the whole RED payload, not the Opus frame. Encrypting
  it as one block hid the RED headers from the SFU, which unwraps RED for
  participants without it: audio through the SFU was 98 to 100% concealed.
  The worker now encrypts each RED block on its own and keeps the headers
  readable (`encryptRedFrame`, `decryptRedFrame`). It recognises RED by the
  frame's `mimeType` metadata.

## Operational notes

- **Browser support.** `RTCRtpScriptTransform` exists in Chrome and Edge 141,
  Firefox 117 and Safari 15.4 and later. Older browsers are refused with a
  message, never allowed to send in the clear. Firefox and Safari were not
  tested; only Chromium 158 was.
- **Key id clashes.** Media key ids are 32 random bits. Two senders with the
  same id are handled (each candidate key is tried) at the cost of an extra
  decryption attempt.
- **Two tabs, one profile.** Both share the IndexedDB device key. If both
  create it at the same moment on first use, the last write wins and one tab
  uses a key that is not stored; contacts then see a new device once.
- **Rotation cost.** Each join or leave sends one wrapped key per pair (N-1
  messages per participant) and switches keys one second later. Frames in that
  second use the old key, which receivers keep (`KEYS_KEPT_PER_SENDER`).
- **Recording, transcription, server-side mixing** are impossible by design
  while calls are end-to-end encrypted. A feature that needs the server to see
  media must be an explicit opt-out per call.

## What was verified, and how

Headless Chromium 158 with fake camera and microphone, one context per user,
against a local stack (SFU, Keycloak, Postgres, Redis):

- 2 and 3 participants on direct links, 3 forced through the SFU, 5 through the
  SFU then down to 3 on direct links: every video plays, the badge reads
  **End-to-end encrypted**, every sender and receiver has a transform, every
  received frame decrypts in the worker (no drops), and only VP8 and Opus are
  negotiated.
- Leave and rejoin twice, on both paths: video keeps playing across the key
  rotations.
- A browser without `RTCRtpScriptTransform` sees the refusal and does not join.
- Simulcast through the SFU with encryption on: each camera sends three layers
  (320, 640, 1280 pixels wide), every receiver gets the full layer, and no frame
  fails to decrypt. (Measured while a room-size rule still lowered the layer in
  bigger rooms; the full-layer check is below.)
- Screen sharing on both paths: every other participant plays the screen.
  (Measured with the 2.5 Mbit/s screen cap that was removed afterwards.)

Network conditions, with one browser behind a veth pair in its own network
namespace so `tc` could shape its link alone:

- A receiver capped at 600 kbit/s steps down to the quarter layer while the
  others keep the full layer, and climbs back to full within about 30 seconds
  of the cap being lifted.
- 20% random loss alone does not step a receiver down: retransmission (NACK)
  repairs it on a short link, so the receiver reports almost no loss, and the
  picture stays full.
- A sender whose uplink is capped at 700 kbit/s stops sending its full layer;
  receivers get its half layer without a freeze, and the full layer again once
  the cap is lifted. Other senders stay at full.
- A direct link capped at 300 kbit/s: the sender's single stream drops to 640
  wide at 120 to 220 kbit/s with no packet loss, and recovers when the cap is
  lifted.

Also verified:

- TURN relay (`?relay=true`) on both paths, 3 participants: every connection
  uses a relay candidate, video plays and decrypts, and coturn logs
  allocations for usernames of the form `<expiry>:<user id>`.
- Firefox 160 with Chromium in one call, on both paths: both play each other's
  video, encrypted.
- Device trust, before vouching replaced trust on first use: two browsers that
  met before showed **End-to-end encrypted**; when one joined again from a new
  browser profile, the other showed **Encrypted · check devices**.
- Vouching with the real Keycloak 26.7.4 realm, 3 participants through the
  SFU: every device gets a DPoP-bound token and every badge shows
  **End-to-end encrypted**. With one browser unable to reach the silent login,
  the others show **Encrypted · unverified devices**, send it nothing and keep
  playing each other; its own badge says the same and it plays nothing. With no
  `e2ee.json`, keys flow and every badge says the devices are unverified.

- The vouching round trip with Keycloak on another site than the app
  (`127.0.0.1` against `localhost`) and third-party cookies blocked: Chromium
  (`--test-third-party-cookie-phaseout`) 6 of 6 runs with two people and 6 of
  6 with three, on both paths; Firefox 160 (`network.cookie.cookieBehavior` 1)
  with Chromium on both paths. Every badge **End-to-end encrypted**. The
  three-person runs were clean at a normal machine load; the earlier failures
  happened at a load average of 60 to 95 from another build.

Not verified: Safari, and Firefox inside the test network namespace (it
gathered no ICE candidates there; on the host network it works).
