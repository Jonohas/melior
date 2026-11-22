import { type CandidateCounts, countCandidate, NO_CANDIDATES } from './debug-candidates';
import { appendLog, type DebugDirection, type DebugLogEntry, formatSeconds } from './debug-log';
import type { SessionId } from './session-id';
import type { ClientMessage, IceServer, Participant, ServerMessage } from './signalling-message';

/** Messages one way and the other. */
export type InOut = { in: number; out: number };

const NOTHING: InOut = { in: 0, out: 0 };

/** What the signalling said about one peer's direct link. */
export type PeerSignals = {
    /** The peer's username and the order it was first seen in; never its session id. */
    label: string;
    offers: InOut;
    answers: InOut;
    ice: InOut;
    /** The candidates this side sent for the link, and those the peer sent. */
    localCandidates: CandidateCounts;
    remoteCandidates: CandidateCounts;
    /** When the last `route` to direct arrived, and its epoch. */
    toDirectAt: number | null;
    toDirectEpoch: number | null;
    /** When this side reported that route live, or null while it has not. */
    liveAt: number | null;
};

/** Everything the debug panel learns from the signalling, folded message by message. */
export type CallSignals = {
    log: DebugLogEntry[];
    /** How many lines were logged so far, the dropped ones included. */
    logged: number;
    peers: Record<SessionId, PeerSignals>;
    /** How many peers were labelled so far. */
    labelled: number;
    /** The TURN and STUN URLs of the last `session`; null before one. Never the username or credential. */
    iceServerUrls: string[] | null;
    /** The last `uplink`; null before one (the SFU uplink then sends). */
    uplinkActive: boolean | null;
    /** Whose tracks the SFU sends this side, by the last server `sdp_offer`. */
    sfuPeers: SessionId[];
    sfuCandidates: { local: CandidateCounts; remote: CandidateCounts };
    /** How many `session` messages arrived: one per connection, so reconnects count up. */
    sessions: number;
};

export const EMPTY_SIGNALS: CallSignals = {
    log: [],
    logged: 0,
    peers: {},
    labelled: 0,
    iceServerUrls: null,
    uplinkActive: null,
    sfuPeers: [],
    sfuCandidates: { local: NO_CANDIDATES, remote: NO_CANDIDATES },
    sessions: 0,
};

/** Only the URLs: the TURN username and credential never leave the signalling message. */
export const iceServerUrls = (servers: readonly IceServer[]): string[] => servers.flatMap((server) => server.urls);

const log = (state: CallSignals, direction: DebugDirection, text: string, at: number): CallSignals => ({
    ...state,
    logged: state.logged + 1,
    log: appendLog(state.log, { seq: state.logged + 1, at, direction, text }),
});

/** The peer's entry, created (and labelled) the first time it is named. */
const withPeer = (state: CallSignals, peer: SessionId, name = 'unknown'): CallSignals => {
    if (state.peers[peer]) return state;
    const labelled = state.labelled + 1;
    const entry: PeerSignals = {
        label: `${name} #${labelled}`,
        offers: NOTHING,
        answers: NOTHING,
        ice: NOTHING,
        localCandidates: NO_CANDIDATES,
        remoteCandidates: NO_CANDIDATES,
        toDirectAt: null,
        toDirectEpoch: null,
        liveAt: null,
    };
    return { ...state, labelled, peers: { ...state.peers, [peer]: entry } };
};

const updatePeer = (
    state: CallSignals,
    peer: SessionId,
    change: (entry: PeerSignals) => Partial<PeerSignals>,
): CallSignals => {
    const known = withPeer(state, peer);
    const entry = known.peers[peer];
    if (!entry) return known;
    return { ...known, peers: { ...known.peers, [peer]: { ...entry, ...change(entry) } } };
};

const labelOf = (state: CallSignals, peer: SessionId): string => state.peers[peer]?.label ?? 'unknown peer';

const bump = (counts: InOut, direction: keyof InOut): InOut => ({ ...counts, [direction]: counts[direction] + 1 });

const withParticipants = (state: CallSignals, participants: readonly Participant[]): CallSignals =>
    participants.reduce((next, participant) => withPeer(next, participant.session_id, participant.username), state);

/** How a direct attempt went, for the log line of the `route` or `link_close` that ends it. */
export const attemptSummary = (entry: PeerSignals | undefined, at: number): string => {
    if (!entry || entry.toDirectAt === null) return '';
    const since = `${formatSeconds(at - entry.toDirectAt)} after route direct (epoch ${entry.toDirectEpoch})`;
    if (entry.liveAt === null) return ` — ${since}, never live here`;
    return ` — ${since}, live here after ${formatSeconds(entry.liveAt - entry.toDirectAt)}`;
};

type Handler<P> = (state: CallSignals, payload: P, at: number) => CallSignals;
type Handlers<M extends { channel: string; payload: unknown }> = {
    [C in M['channel']]?: Handler<Extract<M, { channel: C }>['payload']>;
};

const onSession: Handler<Extract<ServerMessage, { channel: 'session' }>['payload']> = (state, payload, at) => {
    const urls = iceServerUrls(payload.ice_servers);
    const counted = { ...state, sessions: state.sessions + 1, iceServerUrls: urls };
    const next = withParticipants(counted, payload.participants);
    const servers = urls.length > 0 ? urls.join(', ') : 'none';
    const others = payload.participants.length;
    return log(next, 'in', `session #${next.sessions}: joined with ${others} others; ICE servers: ${servers}`, at);
};

const onRoute: Handler<Extract<ServerMessage, { channel: 'route' }>['payload']> = (state, route, at) => {
    const { peer, path, epoch } = route;
    const summary = path === 'forwarded' ? attemptSummary(state.peers[peer], at) : '';
    const toDirect = { toDirectAt: at, toDirectEpoch: epoch, liveAt: null };
    const next = path === 'direct' ? updatePeer(state, peer, () => toDirect) : withPeer(state, peer);
    return log(next, 'in', `route ${labelOf(next, peer)} → ${path} (epoch ${epoch})${summary}`, at);
};

const onLinkClose: Handler<{ peer: SessionId }> = (state, { peer }, at) => {
    const summary = attemptSummary(state.peers[peer], at);
    return log(state, 'in', `link_close ${labelOf(state, peer)} (the message carries no reason)${summary}`, at);
};

const onServerOffer: Handler<Extract<ServerMessage, { channel: 'sdp_offer' }>['payload']> = (state, offer, at) => {
    const sfuPeers = [...new Set(offer.tracks.map((track) => track.session_id))];
    const carried = sfuPeers.map((peer) => labelOf(state, peer)).join(', ') || 'nobody';
    const text = `sdp_offer from the SFU: ${offer.tracks.length} tracks, of ${carried}`;
    return log({ ...state, sfuPeers }, 'in', text, at);
};

const RECEIVED: Handlers<ServerMessage> = {
    session: onSession,
    peer_joined: (state, payload, at) => {
        const next = withPeer(state, payload.session_id, payload.username);
        return log(next, 'in', `peer_joined ${labelOf(next, payload.session_id)}`, at);
    },
    peer_left: (state, { session_id }, at) => {
        const { [session_id]: _gone, ...peers } = state.peers;
        return log({ ...state, peers }, 'in', `peer_left ${labelOf(state, session_id)}`, at);
    },
    sdp_offer: onServerOffer,
    sdp_answer: (state, _payload, at) => log(state, 'in', 'sdp_answer from the SFU', at),
    ice: (state, payload) => ({
        ...state,
        sfuCandidates: {
            ...state.sfuCandidates,
            remote: countCandidate(state.sfuCandidates.remote, payload.candidate),
        },
    }),
    uplink: (state, { active }, at) =>
        log({ ...state, uplinkActive: active }, 'in', `uplink ${active ? 'resumed' : 'paused'}`, at),
    route: onRoute,
    link_close: onLinkClose,
    peer_offer: (state, { from }, at) => {
        const next = updatePeer(state, from, (entry) => ({ offers: bump(entry.offers, 'in') }));
        return log(next, 'in', `peer_offer from ${labelOf(next, from)}`, at);
    },
    peer_answer: (state, { from }, at) => {
        const next = updatePeer(state, from, (entry) => ({ answers: bump(entry.answers, 'in') }));
        return log(next, 'in', `peer_answer from ${labelOf(next, from)}`, at);
    },
    peer_ice: (state, { from, candidate }) =>
        updatePeer(state, from, (entry) => ({
            ice: bump(entry.ice, 'in'),
            remoteCandidates: countCandidate(entry.remoteCandidates, candidate.candidate),
        })),
    closed: (state, { reason }, at) => log(state, 'in', `closed: ${reason}`, at),
    error: (state, { message }, at) => log(state, 'in', `error: ${message}`, at),
};

/** Folds a server message into the signals. Chat, voice state and key messages change nothing. */
export const recordReceived = (state: CallSignals, message: ServerMessage, at: number): CallSignals => {
    const handler = RECEIVED[message.channel] as Handler<ServerMessage['payload']> | undefined;
    return handler ? handler(state, message.payload, at) : state;
};

const onRouteLive: Handler<{ peer: SessionId; epoch: number }> = (state, { peer, epoch }, at) => {
    const startedAt = state.peers[peer]?.toDirectEpoch === epoch ? state.peers[peer]?.toDirectAt : null;
    const forDirect = startedAt !== null && startedAt !== undefined;
    const next = forDirect ? updatePeer(state, peer, () => ({ liveAt: at })) : state;
    const after = forDirect ? `, ${formatSeconds(at - startedAt)} after route direct` : '';
    return log(next, 'out', `route_live ${labelOf(next, peer)} (epoch ${epoch})${after}`, at);
};

const SENT: Handlers<ClientMessage> = {
    sdp_offer: (state, _payload, at) => log(state, 'out', 'sdp_offer to the SFU', at),
    sdp_answer: (state, _payload, at) => log(state, 'out', 'sdp_answer to the SFU', at),
    ice: (state, payload) => ({
        ...state,
        sfuCandidates: { ...state.sfuCandidates, local: countCandidate(state.sfuCandidates.local, payload.candidate) },
    }),
    route_live: onRouteLive,
    link_failed: (state, { peer }, at) => log(state, 'out', `link_failed ${labelOf(state, peer)}`, at),
    peer_offer: (state, { to }, at) => {
        const next = updatePeer(state, to, (entry) => ({ offers: bump(entry.offers, 'out') }));
        return log(next, 'out', `peer_offer to ${labelOf(next, to)}`, at);
    },
    peer_answer: (state, { to }, at) => {
        const next = updatePeer(state, to, (entry) => ({ answers: bump(entry.answers, 'out') }));
        return log(next, 'out', `peer_answer to ${labelOf(next, to)}`, at);
    },
    peer_ice: (state, { to, candidate }) =>
        updatePeer(state, to, (entry) => ({
            ice: bump(entry.ice, 'out'),
            localCandidates: countCandidate(entry.localCandidates, candidate.candidate),
        })),
};

/** Folds a message this side sent into the signals. Chat, voice state and key messages change nothing. */
export const recordSent = (state: CallSignals, message: ClientMessage, at: number): CallSignals => {
    const handler = SENT[message.channel] as Handler<ClientMessage['payload']> | undefined;
    return handler ? handler(state, message.payload, at) : state;
};

/** A line of what this browser saw itself, such as a connection state change. */
export const recordNote = (state: CallSignals, text: string, at: number): CallSignals => log(state, 'local', text, at);
