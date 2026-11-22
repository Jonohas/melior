import { describe, expect, test } from 'vitest';
import {
    formatBitrate,
    formatBytes,
    formatCandidate,
    formatCounts,
    formatInOut,
    formatPair,
    timeOfDay,
} from './debug-format';
import { formatSeconds } from './debug-log';

describe('debug formatting', () => {
    test('bytes', () => {
        expect(formatBytes(0)).toBe('0 B');
        expect(formatBytes(512)).toBe('512 B');
        expect(formatBytes(1500)).toBe('1.5 kB');
        expect(formatBytes(12_345_678)).toBe('12.3 MB');
    });

    test('seconds', () => {
        expect(formatSeconds(10_000)).toBe('10.0 s');
        expect(formatSeconds(-5)).toBe('0.0 s');
    });

    test('bitrate, counts and message counts', () => {
        expect(formatBitrate(null)).toBe('—');
        expect(formatBitrate({ inKbps: 900, outKbps: 50 })).toBe('in 900 kbit/s, out 50 kbit/s');
        expect(formatCounts({ host: 2, srflx: 1, prflx: 0, relay: 0 })).toBe('host 2 · srflx 1 · prflx 0 · relay 0');
        expect(formatCounts(null)).toBe('—');
        expect(formatInOut({ in: 1, out: 3 })).toBe('1 in / 3 out');
    });

    test('candidates and pairs', () => {
        const relay = { type: 'relay', protocol: 'udp', address: '198.51.100.2', port: 3478, relayProtocol: 'tls' };
        const host = { type: 'host', protocol: 'udp', address: null, port: null, relayProtocol: null };
        expect(formatCandidate(relay)).toBe('relay udp 198.51.100.2:3478 (via tls)');
        expect(formatCandidate(host)).toBe('host udp');
        expect(formatPair(null)).toBe('none selected');
        expect(formatPair({ local: relay, remote: host, state: 'succeeded', rttMs: 40 })).toBe(
            'relay udp 198.51.100.2:3478 (via tls) ⇄ host udp, rtt 40 ms',
        );
    });

    test('time of day', () => {
        expect(timeOfDay('2026-11-22T13:32:12.345Z')).toBe('13:32:12.345');
    });
});
