import { describe, expect, test } from 'vitest';
import type { E2eeSession } from '@/features/conference/api';
import { type ConferenceConnection, releaseConnection, withE2ee } from './session-messages';

const connection = (): ConferenceConnection => ({
    signalling: null,
    peer: null,
    mesh: null,
    transforms: null,
    e2ee: null,
    e2eeWaiting: null,
});

const exchange = (log: string[]): E2eeSession => ({
    addPeer: (peer) => log.push(`add ${peer}`),
    removePeer: (peer) => log.push(`remove ${peer}`),
    handle: async (from) => {
        log.push(`handle ${from}`);
    },
    close: () => log.push('close'),
});

describe('withE2ee', () => {
    test('runs at once when the key exchange exists', () => {
        const log: string[] = [];
        const open = { ...connection(), e2ee: exchange(log) };
        withE2ee(open, (e2ee) => e2ee.addPeer('b', 'bob', true));
        expect(log).toEqual(['add b']);
    });

    test('holds key work while the keys are being created, in order, and drops it when nothing starts', () => {
        const log: string[] = [];
        const waiting: Array<(e2ee: E2eeSession) => void> = [];
        const starting = { ...connection(), e2eeWaiting: waiting };
        withE2ee(starting, (e2ee) => void e2ee.handle('a', 'x'));
        withE2ee(starting, (e2ee) => e2ee.removePeer('a'));
        expect(log).toEqual([]);
        const ready = exchange(log);
        for (const action of waiting.splice(0)) action(ready);
        expect(log).toEqual(['handle a', 'remove a']);

        const idle = connection();
        withE2ee(idle, (e2ee) => e2ee.addPeer('c', 'carol', true));
        releaseConnection(idle);
        expect(log).toEqual(['handle a', 'remove a']);
    });
});
