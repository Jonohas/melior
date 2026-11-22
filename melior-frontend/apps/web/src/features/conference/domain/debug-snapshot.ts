import type { CandidateCounts } from './debug-candidates';
import { inferLinkRoute, type LinkRoute } from './debug-link-route';
import { type DebugDirection, formatSeconds } from './debug-log';
import type { CallSignals, InOut, PeerSignals } from './debug-signals';
import type { Bitrate, ConnectionStates, TransportStats } from './debug-stats';
import { type RoomMediaPath, roomMediaPath } from './room-path';
import type { SessionId } from './session-id';
import type { Path } from './signalling-message';
import type { PeerRouteView } from './source-selection';

/** One connection as sampled: its states, its transport stats, and the bitrate since the previous sample. */
export type ConnectionSample = { states: ConnectionStates; transport: TransportStats; bitrate: Bitrate | null };

/** A peer the mesh knows: its route view, and its direct link when one is open. */
export type LinkSample = { peer: SessionId; view: PeerRouteView; link: ConnectionSample | null };

export type SnapshotInput = {
    now: number;
    forceRelay: boolean;
    sfu: ConnectionSample | null;
    links: readonly LinkSample[];
};

export type ConnectionDebug = ConnectionStates & TransportStats & { bitrate: Bitrate | null };

export type PeerDebug = {
    /** Username and order of appearance; never the session id. */
    peer: string;
    route: LinkRoute;
    epoch: number | null;
    preferredPath: Path;
    shownPath: Path;
    link: ConnectionDebug | null;
    localCandidates: CandidateCounts | null;
    remoteCandidates: CandidateCounts | null;
    offers: InOut | null;
    answers: InOut | null;
    ice: InOut | null;
    /** Time since the last route to direct, e.g. `12.3 s`; null without one. */
    sinceToDirect: string | null;
    /** How long the route to direct took to go live here, or `not live`; null without a route to direct. */
    liveAfter: string | null;
};

export type DebugLogLine = { seq: number; time: string; direction: DebugDirection; text: string };

export type RoomDebug = {
    /** What the routes ask for, over all peers: `direct`, `forwarded`, `mixed …` or `alone`. */
    requestedPath: string;
    /** What the peers are shown over. */
    shownPath: string;
    uplink: 'sending' | 'paused';
    /** Null before the first `session` message. */
    iceServerUrls: string[] | null;
    turnProvided: boolean;
    forceRelay: boolean;
    /** Connections so far: more than one means the call reconnected. */
    sessions: number;
};

/** The whole panel, as plain data: shown as it is and copied as JSON. Holds no session id, token or credential. */
export type CallDebugSnapshot = {
    takenAt: string;
    room: RoomDebug;
    sfu: (ConnectionDebug & { localCandidates: CandidateCounts; remoteCandidates: CandidateCounts }) | null;
    peers: PeerDebug[];
    log: DebugLogLine[];
};

const describePath = (path: RoomMediaPath): string => {
    switch (path.kind) {
        case 'none':
            return 'alone';
        case 'peer_to_peer':
            return 'direct';
        case 'via_server':
            return 'forwarded';
        case 'mixed':
            return `mixed (${path.direct} of ${path.total} direct)`;
    }
};

const roomPaths = (links: readonly LinkSample[], pick: (view: PeerRouteView) => Path): string => {
    const paths = Object.fromEntries(links.map((link) => [link.peer, pick(link.view)]));
    return describePath(roomMediaPath(Object.keys(paths), paths));
};

const isTurn = (url: string): boolean => /^turns?:/i.test(url);

const connectionDebug = (sample: ConnectionSample): ConnectionDebug => ({
    ...sample.states,
    ...sample.transport,
    bitrate: sample.bitrate,
});

const timings = (signals: PeerSignals | undefined, now: number) => {
    if (!signals || signals.toDirectAt === null) return { sinceToDirect: null, liveAfter: null };
    const live = signals.liveAt === null ? 'not live' : formatSeconds(signals.liveAt - signals.toDirectAt);
    return { sinceToDirect: formatSeconds(now - signals.toDirectAt), liveAfter: live };
};

const peerDebug = (sample: LinkSample, signals: CallSignals, now: number): PeerDebug => {
    const entry = signals.peers[sample.peer];
    const facts = { view: sample.view, linkOpen: sample.link !== null };
    return {
        peer: entry?.label ?? 'unknown peer',
        route: inferLinkRoute({ ...facts, sfuCarriesPeer: signals.sfuPeers.includes(sample.peer) }),
        epoch: sample.view.epoch,
        preferredPath: sample.view.preferred,
        shownPath: sample.view.shown,
        link: sample.link ? connectionDebug(sample.link) : null,
        localCandidates: entry?.localCandidates ?? null,
        remoteCandidates: entry?.remoteCandidates ?? null,
        offers: entry?.offers ?? null,
        answers: entry?.answers ?? null,
        ice: entry?.ice ?? null,
        ...timings(entry, now),
    };
};

const sfuCandidates = ({ sfuCandidates: { local, remote } }: CallSignals) => ({
    localCandidates: local,
    remoteCandidates: remote,
});

/** Puts the signals and a stats sample together into what the debug panel shows. */
export const buildDebugSnapshot = (signals: CallSignals, input: SnapshotInput): CallDebugSnapshot => {
    const { now, sfu, links } = input;
    const urls = signals.iceServerUrls;
    return {
        takenAt: new Date(now).toISOString(),
        room: {
            requestedPath: roomPaths(links, (view) => view.preferred),
            shownPath: roomPaths(links, (view) => view.shown),
            uplink: signals.uplinkActive === false ? 'paused' : 'sending',
            iceServerUrls: urls,
            turnProvided: (urls ?? []).some(isTurn),
            forceRelay: input.forceRelay,
            sessions: signals.sessions,
        },
        sfu: sfu ? { ...connectionDebug(sfu), ...sfuCandidates(signals) } : null,
        peers: links.map((link) => peerDebug(link, signals, now)),
        log: signals.log.map(({ at, ...line }) => ({ ...line, time: new Date(at).toISOString() })),
    };
};

/** Keys that never go into a copied report, wherever they appear. */
const SECRET_KEYS = new Set(['username', 'credential', 'password', 'token', 'session_id', 'sessionId']);

/**
 * The report a user pastes into an issue: the snapshot and whatever the caller adds (the channel's media
 * settings), as indented JSON. Credentials, tokens and session ids are dropped by key as a second line of defence.
 */
export const debugReport = (snapshot: CallDebugSnapshot, extra: Record<string, unknown> = {}): string =>
    JSON.stringify({ ...extra, ...snapshot }, (key, value) => (SECRET_KEYS.has(key) ? undefined : value), 2);
