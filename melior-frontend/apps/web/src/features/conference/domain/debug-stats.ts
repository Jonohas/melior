/** One entry of an `RTCStatsReport`, as a plain object; the domain reads only the fields it knows. */
export type StatsEntry = Readonly<Record<string, unknown>>;

/** One end of the candidate pair a connection uses. */
export type CandidateInfo = {
    /** `host`, `srflx`, `prflx` or `relay`. */
    type: string | null;
    /** `udp` or `tcp`. */
    protocol: string | null;
    address: string | null;
    port: number | null;
    /** For a relay candidate: how this browser reaches the TURN server (`udp`, `tcp`, `tls`). */
    relayProtocol: string | null;
};

export type SelectedPair = {
    local: CandidateInfo | null;
    remote: CandidateInfo | null;
    state: string | null;
    /** The last round-trip time on the pair, in milliseconds. */
    rttMs: number | null;
};

/** What a connection's stats say about its transport. */
export type TransportStats = {
    selectedPair: SelectedPair | null;
    bytesSent: number;
    bytesReceived: number;
};

/** The connection states the debug panel shows, as the browser reports them. */
export type ConnectionStates = {
    connectionState: string;
    iceConnectionState: string;
    iceGatheringState: string;
};

const text = (entry: StatsEntry | undefined, key: string): string | null => {
    const value = entry?.[key];
    return typeof value === 'string' ? value : null;
};

const count = (entry: StatsEntry | undefined, key: string): number | null => {
    const value = entry?.[key];
    return typeof value === 'number' && Number.isFinite(value) ? value : null;
};

const candidateInfo = (entry: StatsEntry | undefined): CandidateInfo | null => {
    if (!entry) return null;
    return {
        type: text(entry, 'candidateType'),
        protocol: text(entry, 'protocol'),
        address: text(entry, 'address') ?? text(entry, 'ip'),
        port: count(entry, 'port'),
        relayProtocol: text(entry, 'relayProtocol'),
    };
};

/**
 * The pair in use: the one a `transport` entry names (Chromium, Safari), else the pair marked `selected`
 * (Firefox), else a nominated pair that succeeded.
 */
const findSelectedPair = (entries: readonly StatsEntry[]): StatsEntry | undefined => {
    const byId = (id: string | null) => entries.find((entry) => entry.type === 'candidate-pair' && entry.id === id);
    for (const transport of entries.filter((entry) => entry.type === 'transport')) {
        const named = byId(text(transport, 'selectedCandidatePairId'));
        if (named) return named;
    }
    const pairs = entries.filter((entry) => entry.type === 'candidate-pair');
    return (
        pairs.find((pair) => pair.selected === true) ??
        pairs.find((pair) => pair.nominated === true && pair.state === 'succeeded')
    );
};

const describePair = (entries: readonly StatsEntry[], pair: StatsEntry): SelectedPair => {
    const byId = (id: string | null) => entries.find((entry) => entry.id === id);
    const rtt = count(pair, 'currentRoundTripTime');
    return {
        local: candidateInfo(byId(text(pair, 'localCandidateId'))),
        remote: candidateInfo(byId(text(pair, 'remoteCandidateId'))),
        state: text(pair, 'state'),
        rttMs: rtt === null ? null : Math.round(rtt * 1000),
    };
};

const sumOf = (entries: readonly StatsEntry[], key: string): number =>
    entries.reduce((total, entry) => total + (count(entry, key) ?? 0), 0);

/**
 * Sums up a connection's stats report: the candidate pair in use and the bytes moved. Bytes come from the
 * `transport` entries where the browser has them, else from the selected pair.
 */
export const summarizeTransport = (entries: readonly StatsEntry[]): TransportStats => {
    const pair = findSelectedPair(entries);
    const transports = entries.filter((entry) => entry.type === 'transport');
    const counted = transports.length > 0 ? transports : [pair].filter((entry) => entry !== undefined);
    return {
        selectedPair: pair ? describePair(entries, pair) : null,
        bytesSent: sumOf(counted, 'bytesSent'),
        bytesReceived: sumOf(counted, 'bytesReceived'),
    };
};

/** A byte counter sampled at a time in milliseconds. */
export type ByteSample = { at: number; bytesSent: number; bytesReceived: number };

export type Bitrate = { inKbps: number; outKbps: number };

const kbps = (bytes: number, ms: number): number => Math.max(0, Math.round((bytes * 8) / ms));

/** The bitrate between two samples of the same connection. Null without a previous sample or time between them. */
export const bitrateBetween = (previous: ByteSample | undefined, next: ByteSample): Bitrate | null => {
    if (!previous) return null;
    const ms = next.at - previous.at;
    if (ms <= 0) return null;
    return {
        inKbps: kbps(next.bytesReceived - previous.bytesReceived, ms),
        outKbps: kbps(next.bytesSent - previous.bytesSent, ms),
    };
};

const STATE_KEYS: readonly (keyof ConnectionStates)[] = ['connectionState', 'iceConnectionState', 'iceGatheringState'];

/** One line per state that changed between two samples, e.g. `iceConnectionState checking → failed`. */
export const stateChanges = (previous: ConnectionStates | undefined, next: ConnectionStates): string[] => {
    if (!previous) return STATE_KEYS.map((key) => `${key} ${next[key]}`);
    return STATE_KEYS.filter((key) => previous[key] !== next[key]).map(
        (key) => `${key} ${previous[key]} → ${next[key]}`,
    );
};
