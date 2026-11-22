import { describe, expect, test } from 'vitest';
import { candidateTypeOf, countCandidate, NO_CANDIDATES } from './debug-candidates';

const HOST = 'candidate:1 1 udp 2122260223 192.168.1.4 54321 typ host generation 0 ufrag abcd';
const SRFLX = 'candidate:2 1 udp 1686052607 203.0.113.9 40000 typ srflx raddr 192.168.1.4 rport 54321';
const RELAY = 'candidate:3 1 udp 41885439 198.51.100.2 3478 typ relay raddr 203.0.113.9 rport 40000';

describe('candidateTypeOf', () => {
    test('reads the type of a candidate line', () => {
        expect(candidateTypeOf(HOST)).toBe('host');
        expect(candidateTypeOf(SRFLX)).toBe('srflx');
        expect(candidateTypeOf(RELAY)).toBe('relay');
        expect(candidateTypeOf('candidate:4 1 tcp 1 10.0.0.1 9 typ prflx tcptype active')).toBe('prflx');
    });

    test('the end-of-candidates marker and garbage have no type', () => {
        for (const line of [undefined, '', 'candidate:1 1 udp 1 10.0.0.1 9 typ bogus', 'typhost']) {
            expect(candidateTypeOf(line)).toBeNull();
        }
    });
});

describe('countCandidate', () => {
    test('counts per type', () => {
        const counts = [HOST, HOST, SRFLX, RELAY].reduce(countCandidate, NO_CANDIDATES);
        expect(counts).toEqual({ host: 2, srflx: 1, prflx: 0, relay: 1 });
    });

    test('a candidate without a type changes nothing', () => {
        expect(countCandidate(NO_CANDIDATES, '')).toBe(NO_CANDIDATES);
    });
});
