import { describe, expect, test } from 'vitest';
import type { ChannelId } from './channel-id';
import { EMPTY_SIGNALS, recordReceived } from './debug-signals';
import { buildDebugSnapshot, type ConnectionSample, debugReport } from './debug-snapshot';
import type { ServerMessage } from './signalling-message';
import { initialRouteView } from './source-selection';

const BOB = 'bob-session-0123456789';
const CAROL = 'carol-session-0123456789';

const session: ServerMessage = {
    channel: 'session',
    payload: {
        session_id: 'self-session-0123456789',
        channel_id: 'room' as ChannelId,
        username: 'alice',
        participants: [
            { session_id: BOB, username: 'bob', voice: { muted: false, camera: true, screen: false, speaking: false } },
            {
                session_id: CAROL,
                username: 'carol',
                voice: { muted: false, camera: true, screen: false, speaking: false },
            },
        ],
        ice_servers: [{ urls: ['turn:turn.example.org:3478'], username: '1700000000:user-1', credential: 's3cr3t' }],
    },
};

const connection: ConnectionSample = {
    states: { connectionState: 'connecting', iceConnectionState: 'checking', iceGatheringState: 'complete' },
    transport: { selectedPair: null, bytesSent: 10, bytesReceived: 20 },
    bitrate: null,
};

const signals = () => {
    const joined = recordReceived(EMPTY_SIGNALS, session, 0);
    return recordReceived(joined, { channel: 'route', payload: { peer: BOB, path: 'direct', epoch: 1 } }, 1000);
};

const snapshot = () =>
    buildDebugSnapshot(signals(), {
        now: 4000,
        forceRelay: true,
        sfu: connection,
        links: [
            { peer: BOB, view: { ...initialRouteView(), preferred: 'direct', epoch: 1 }, link: connection },
            { peer: CAROL, view: initialRouteView(), link: null },
        ],
    });

describe('buildDebugSnapshot', () => {
    test('the room shows what the routes ask for, the ICE servers and the relay flag', () => {
        expect(snapshot().room).toEqual({
            requestedPath: 'mixed (1 of 2 direct)',
            shownPath: 'forwarded',
            uplink: 'sending',
            iceServerUrls: ['turn:turn.example.org:3478'],
            turnProvided: true,
            forceRelay: true,
            sessions: 1,
        });
    });

    test('a peer shows its inferred route, link states and timings', () => {
        const [bob, carol] = snapshot().peers;
        expect(bob).toMatchObject({
            peer: 'bob #1',
            route: 'ToDirect',
            epoch: 1,
            sinceToDirect: '3.0 s',
            liveAfter: 'not live',
            link: { iceConnectionState: 'checking', bytesReceived: 20 },
        });
        expect(carol).toMatchObject({ peer: 'carol #2', route: 'Forwarded', link: null, sinceToDirect: null });
    });

    test('alone, with no session yet, nothing is assumed', () => {
        const empty = buildDebugSnapshot(EMPTY_SIGNALS, { now: 0, forceRelay: false, sfu: null, links: [] });
        expect(empty.room).toMatchObject({ requestedPath: 'alone', iceServerUrls: null, turnProvided: false });
        expect(empty.sfu).toBeNull();
    });
});

describe('debugReport', () => {
    test('is JSON without session ids, TURN usernames or credentials', () => {
        const report = debugReport(snapshot(), { channelMedia: { mediaMode: 'auto' } });
        const parsed = JSON.parse(report);
        expect(parsed.channelMedia).toEqual({ mediaMode: 'auto' });
        expect(parsed.peers).toHaveLength(2);
        for (const secret of [BOB, CAROL, 'self-session', 's3cr3t', '1700000000:user-1']) {
            expect(report).not.toContain(secret);
        }
    });

    test('drops secret keys wherever the caller put them', () => {
        const report = debugReport(snapshot(), { extra: { token: 'abc', session_id: 'x', credential: 'y', kept: 1 } });
        expect(JSON.parse(report).extra).toEqual({ kept: 1 });
    });
});
