import { describe, expect, test } from 'vitest';
import { parseServerMessage } from './parse-server-message';

const msg = (channel: string, payload: unknown) => JSON.stringify({ channel, payload });

describe('mesh messages', () => {
    test('parses route', () => {
        expect(parseServerMessage(msg('route', { peer: 'p', path: 'direct', epoch: 3 }))).toEqual({
            channel: 'route',
            payload: { peer: 'p', path: 'direct', epoch: 3 },
        });
    });

    test.each([
        { peer: 'p', path: 'sideways', epoch: 1 },
        { peer: 'p', path: 'direct', epoch: -1 },
        { peer: 'p', path: 'direct', epoch: 1.5 },
        { peer: 'p', path: 'direct', epoch: '1' },
        { peer: '', path: 'direct', epoch: 1 },
        { path: 'direct', epoch: 1 },
    ])('rejects route %j', (payload) => {
        expect(parseServerMessage(msg('route', payload))).toBeNull();
    });

    test('parses link_close and uplink', () => {
        expect(parseServerMessage(msg('link_close', { peer: 'p' }))).toEqual({
            channel: 'link_close',
            payload: { peer: 'p' },
        });
        expect(parseServerMessage(msg('uplink', { active: false }))).toEqual({
            channel: 'uplink',
            payload: { active: false },
        });
        expect(parseServerMessage(msg('link_close', {}))).toBeNull();
        expect(parseServerMessage(msg('uplink', { active: 'no' }))).toBeNull();
        expect(parseServerMessage(msg('uplink', null))).toBeNull();
    });

    test('parses relayed offers, answers and candidates', () => {
        expect(parseServerMessage(msg('peer_offer', { from: 'p', sdp: 'v=0' }))).toEqual({
            channel: 'peer_offer',
            payload: { from: 'p', sdp: 'v=0' },
        });
        expect(parseServerMessage(msg('peer_answer', { from: 'p', sdp: 'v=0' }))?.channel).toBe('peer_answer');
        const ice = parseServerMessage(msg('peer_ice', { from: 'p', candidate: { candidate: 'c', sdpMid: '0' } }));
        expect(ice?.channel === 'peer_ice' && ice.payload.candidate.candidate).toBe('c');
    });

    test('rejects malformed relays', () => {
        expect(parseServerMessage(msg('peer_offer', { from: 'p' }))).toBeNull();
        expect(parseServerMessage(msg('peer_answer', { sdp: 'x' }))).toBeNull();
        expect(parseServerMessage(msg('peer_ice', { from: 'p', candidate: 'c' }))).toBeNull();
        expect(parseServerMessage(msg('peer_ice', { candidate: {} }))).toBeNull();
    });
});
