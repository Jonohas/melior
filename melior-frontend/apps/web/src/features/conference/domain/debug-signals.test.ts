import { describe, expect, test } from 'vitest';
import type { ChannelId } from './channel-id';
import { DEBUG_LOG_LIMIT } from './debug-log';
import { type CallSignals, EMPTY_SIGNALS, recordNote, recordReceived, recordSent } from './debug-signals';
import type { ClientMessage, ServerMessage } from './signalling-message';

const SELF = 'self-session-0123456789';
const BOB = 'bob-session-0123456789';
const SRFLX = 'candidate:2 1 udp 1686052607 203.0.113.9 40000 typ srflx raddr 192.168.1.4 rport 54321';
const HOST = 'candidate:1 1 udp 2122260223 192.168.1.4 54321 typ host';

const session: ServerMessage = {
    channel: 'session',
    payload: {
        session_id: SELF,
        channel_id: 'room' as ChannelId,
        username: 'alice',
        participants: [
            { session_id: BOB, username: 'bob', voice: { muted: false, camera: true, screen: false, speaking: false } },
        ],
        ice_servers: [
            { urls: ['stun:stun.example.org:3478'] },
            { urls: ['turn:turn.example.org:3478?transport=udp'], username: '1700000000:user-1', credential: 's3cr3t' },
        ],
    },
};

const receive = (state: CallSignals, message: ServerMessage, at = 0) => recordReceived(state, message, at);
const send = (state: CallSignals, message: ClientMessage, at = 0) => recordSent(state, message, at);
const texts = (state: CallSignals) => state.log.map((entry) => entry.text);
const joined = () => receive(EMPTY_SIGNALS, session, 0);

describe('recordReceived', () => {
    test('a session keeps the ICE server URLs only, and labels the peers by name', () => {
        const state = joined();
        expect(state.iceServerUrls).toEqual(['stun:stun.example.org:3478', 'turn:turn.example.org:3478?transport=udp']);
        expect(state.peers[BOB]?.label).toBe('bob #1');
        expect(state.sessions).toBe(1);
        const everything = JSON.stringify(state.log) + JSON.stringify(state.iceServerUrls);
        expect(everything).not.toContain('s3cr3t');
        expect(everything).not.toContain('1700000000:user-1');
    });

    test('the log never names a session id', () => {
        let state = joined();
        state = receive(state, { channel: 'route', payload: { peer: BOB, path: 'direct', epoch: 1 } });
        state = send(state, { channel: 'peer_offer', payload: { to: BOB, sdp: 'v=0' } });
        state = receive(state, { channel: 'link_close', payload: { peer: BOB } });
        state = receive(state, { channel: 'peer_left', payload: { session_id: BOB } });
        expect(JSON.stringify(state.log)).not.toContain(BOB);
        expect(JSON.stringify(state.log)).not.toContain(SELF);
        expect(texts(state)).toContain('peer_left bob #1');
    });

    test('a failed attempt shows how long the route stayed ToDirect and that it never went live', () => {
        let state = joined();
        state = receive(state, { channel: 'route', payload: { peer: BOB, path: 'direct', epoch: 1 } }, 1000);
        state = receive(state, { channel: 'route', payload: { peer: BOB, path: 'forwarded', epoch: 2 } }, 11_000);
        state = receive(state, { channel: 'link_close', payload: { peer: BOB } }, 11_500);
        expect(texts(state).slice(-3)).toEqual([
            'route bob #1 → direct (epoch 1)',
            'route bob #1 → forwarded (epoch 2) — 10.0 s after route direct (epoch 1), never live here',
            'link_close bob #1 (the message carries no reason) — 10.5 s after route direct (epoch 1), never live here',
        ]);
    });

    test('route_live for the direct epoch records when it went live', () => {
        let state = joined();
        state = receive(state, { channel: 'route', payload: { peer: BOB, path: 'direct', epoch: 3 } }, 1000);
        state = send(state, { channel: 'route_live', payload: { peer: BOB, epoch: 3 } }, 2500);
        expect(state.peers[BOB]?.liveAt).toBe(2500);
        expect(texts(state).at(-1)).toBe('route_live bob #1 (epoch 3), 1.5 s after route direct');
        state = receive(state, { channel: 'link_close', payload: { peer: BOB } }, 5000);
        expect(texts(state).at(-1)).toContain('live here after 1.5 s');
    });

    test('route_live for another epoch does not count as live', () => {
        let state = joined();
        state = receive(state, { channel: 'route', payload: { peer: BOB, path: 'direct', epoch: 3 } }, 1000);
        state = send(state, { channel: 'route_live', payload: { peer: BOB, epoch: 2 } }, 2000);
        expect(state.peers[BOB]?.liveAt).toBeNull();
    });

    test('ICE candidates are counted per link and per type, both ways, without a log line each', () => {
        let state = joined();
        const before = state.log.length;
        state = receive(state, { channel: 'peer_ice', payload: { from: BOB, candidate: { candidate: SRFLX } } });
        state = send(state, { channel: 'peer_ice', payload: { to: BOB, candidate: { candidate: HOST } } });
        state = send(state, { channel: 'peer_ice', payload: { to: BOB, candidate: { candidate: '' } } });
        expect(state.peers[BOB]?.ice).toEqual({ in: 1, out: 2 });
        expect(state.peers[BOB]?.remoteCandidates.srflx).toBe(1);
        expect(state.peers[BOB]?.localCandidates).toEqual({ host: 1, srflx: 0, prflx: 0, relay: 0 });
        expect(state.log).toHaveLength(before);
    });

    test('the SFU offer says whose tracks the SFU still sends', () => {
        const tracks = [{ mid: '2', session_id: BOB, source: 'camera' as const }];
        const state = receive(joined(), { channel: 'sdp_offer', payload: { type: 'offer', sdp: 'v=0', tracks } });
        expect(state.sfuPeers).toEqual([BOB]);
        expect(texts(state).at(-1)).toBe('sdp_offer from the SFU: 1 tracks, of bob #1');
    });

    test('uplink, closed and error are logged; chat and voice state are not', () => {
        let state = receive(joined(), { channel: 'uplink', payload: { active: false } });
        expect(state.uplinkActive).toBe(false);
        state = receive(state, {
            channel: 'voice_state',
            payload: { session_id: BOB, muted: true, camera: false, screen: false, speaking: false },
        });
        state = receive(state, { channel: 'closed', payload: { reason: 'room_moved' } });
        expect(texts(state).slice(-2)).toEqual(['uplink paused', 'closed: room_moved']);
    });

    test('the log keeps the last entries only, numbered', () => {
        let state = EMPTY_SIGNALS;
        for (let index = 0; index < DEBUG_LOG_LIMIT + 5; index += 1) state = recordNote(state, `note ${index}`, index);
        expect(state.log).toHaveLength(DEBUG_LOG_LIMIT);
        expect(state.log[0]?.text).toBe('note 5');
        expect(state.log.at(-1)?.seq).toBe(DEBUG_LOG_LIMIT + 5);
    });
});
