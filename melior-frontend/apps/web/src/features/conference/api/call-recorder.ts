import {
    type ByteSample,
    bitrateBetween,
    buildDebugSnapshot,
    type CallDebugSnapshot,
    type CallSignals,
    type ConnectionSample,
    type ConnectionStates,
    EMPTY_SIGNALS,
    type LinkSample,
    recordNote,
    recordReceived,
    recordSent,
    type ServerMessage,
    type StatsEntry,
    stateChanges,
    summarizeTransport,
} from '@/features/conference/domain';
import type { MeshDebugLink } from './mesh';
import type { SignallingClient } from './signalling-client';

/** The connections of the call right now; read at every sample, since a reconnect replaces them all. */
export type DebugSources = {
    forceRelay: boolean;
    sfu: RTCPeerConnection | null;
    links: readonly MeshDebugLink[];
};

/**
 * Keeps what the debug panel needs for one call. Recording the signalling is cheap and always on, so a panel
 * opened later still shows how the call got where it is; the stats are read only when the panel asks.
 */
export type CallRecorder = {
    received: (message: ServerMessage) => void;
    /** The same client, with every message it sends recorded too. */
    tap: (client: SignallingClient) => SignallingClient;
    /** Forgets everything: a new call starts. */
    reset: () => void;
    /** Reads every connection's stats once and returns the panel's state. */
    sample: (sources: DebugSources) => Promise<CallDebugSnapshot>;
};

const readStats = async (connection: RTCPeerConnection): Promise<StatsEntry[]> => {
    const entries: StatsEntry[] = [];
    try {
        const report = await connection.getStats();
        report.forEach((entry: StatsEntry) => entries.push(entry));
    } catch {
        // A closing connection has no stats; it shows as empty.
    }
    return entries;
};

const statesOf = (connection: RTCPeerConnection): ConnectionStates => ({
    connectionState: connection.connectionState,
    iceConnectionState: connection.iceConnectionState,
    iceGatheringState: connection.iceGatheringState,
});

type Previous = { states: ConnectionStates; bytes: ByteSample };

export const createCallRecorder = (now: () => number = Date.now): CallRecorder => {
    let signals: CallSignals = EMPTY_SIGNALS;
    // Per connection object, so a new link to the same peer starts its states and bitrate afresh.
    let previous = new WeakMap<RTCPeerConnection, Previous>();

    const sampleConnection = async (connection: RTCPeerConnection, label: string): Promise<ConnectionSample> => {
        const transport = summarizeTransport(await readStats(connection));
        const at = now();
        const states = statesOf(connection);
        const before = previous.get(connection);
        for (const change of stateChanges(before?.states, states))
            signals = recordNote(signals, `${label} ${change}`, at);
        const bytes = { at, bytesSent: transport.bytesSent, bytesReceived: transport.bytesReceived };
        previous.set(connection, { states, bytes });
        return { states, transport, bitrate: bitrateBetween(before?.bytes, bytes) };
    };

    const sampleLink = async (link: MeshDebugLink): Promise<LinkSample> => {
        const label = `direct link to ${signals.peers[link.peer]?.label ?? 'unknown peer'}`;
        const sample = link.connection ? await sampleConnection(link.connection, label) : null;
        return { peer: link.peer, view: link.view, link: sample };
    };

    return {
        received: (message) => {
            signals = recordReceived(signals, message, now());
        },
        tap: (client) => ({
            ...client,
            send: (message) => {
                const sent = client.send(message);
                if (sent) signals = recordSent(signals, message, now());
                return sent;
            },
        }),
        reset: () => {
            signals = EMPTY_SIGNALS;
            previous = new WeakMap();
        },
        sample: async ({ forceRelay, sfu, links }) => {
            const [sfuSample, linkSamples] = await Promise.all([
                sfu ? sampleConnection(sfu, 'SFU') : null,
                Promise.all(links.map(sampleLink)),
            ]);
            return buildDebugSnapshot(signals, { now: now(), forceRelay, sfu: sfuSample, links: linkSamples });
        },
    };
};
