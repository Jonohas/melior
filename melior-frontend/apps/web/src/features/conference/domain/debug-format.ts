import { CANDIDATE_TYPES, type CandidateCounts } from './debug-candidates';
import type { InOut } from './debug-signals';
import type { Bitrate, CandidateInfo, SelectedPair } from './debug-stats';

const UNITS = ['B', 'kB', 'MB', 'GB'] as const;

/** A byte count for people: `0 B`, `512 B`, `1.5 kB`, `12.3 MB`. */
export const formatBytes = (bytes: number): string => {
    let value = Math.max(0, bytes);
    let unit = 0;
    while (value >= 1000 && unit < UNITS.length - 1) {
        value /= 1000;
        unit += 1;
    }
    return unit === 0 ? `${value} B` : `${value.toFixed(1)} ${UNITS[unit]}`;
};

export const formatBitrate = (bitrate: Bitrate | null): string =>
    bitrate ? `in ${bitrate.inKbps} kbit/s, out ${bitrate.outKbps} kbit/s` : '—';

/** `host 2 · srflx 1 · prflx 0 · relay 0`, or a dash when nothing was counted. */
export const formatCounts = (counts: CandidateCounts | null): string =>
    counts ? CANDIDATE_TYPES.map((type) => `${type} ${counts[type]}`).join(' · ') : '—';

export const formatInOut = (counts: InOut | null): string => (counts ? `${counts.in} in / ${counts.out} out` : '—');

/** One end of a candidate pair: `relay udp 203.0.113.7:3478 (via tls)`. */
export const formatCandidate = (candidate: CandidateInfo | null): string => {
    if (!candidate) return '—';
    const where = candidate.address === null ? '' : ` ${candidate.address}:${candidate.port ?? '?'}`;
    const via = candidate.relayProtocol === null ? '' : ` (via ${candidate.relayProtocol})`;
    return `${candidate.type ?? '?'} ${candidate.protocol ?? '?'}${where}${via}`;
};

/** The pair in use, local end first: `srflx udp … ⇄ host udp …`, or `none selected`. */
export const formatPair = (pair: SelectedPair | null): string => {
    if (!pair) return 'none selected';
    const rtt = pair.rttMs === null ? '' : `, rtt ${pair.rttMs} ms`;
    return `${formatCandidate(pair.local)} ⇄ ${formatCandidate(pair.remote)}${rtt}`;
};

/** The time of day of an ISO timestamp, to the millisecond: `14:32:12.345`. */
export const timeOfDay = (iso: string): string => iso.slice(11, 23);
