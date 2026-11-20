import { describe, expect, test } from 'vitest';
import { roomMediaPath } from './room-path';

describe('roomMediaPath', () => {
    test('nobody else in the room has no path', () => {
        expect(roomMediaPath([], {})).toEqual({ kind: 'none' });
        expect(roomMediaPath([], { gone: 'direct' })).toEqual({ kind: 'none' });
    });

    test('everyone shown direct is peer to peer', () => {
        expect(roomMediaPath(['a'], { a: 'direct' })).toEqual({ kind: 'peer_to_peer' });
        expect(roomMediaPath(['a', 'b'], { a: 'direct', b: 'direct' })).toEqual({ kind: 'peer_to_peer' });
    });

    test('everyone shown forwarded, or not shown yet, is via the server', () => {
        expect(roomMediaPath(['a', 'b'], { a: 'forwarded', b: 'forwarded' })).toEqual({ kind: 'via_server' });
        expect(roomMediaPath(['a', 'b'], {})).toEqual({ kind: 'via_server' });
    });

    test('a mix reports how many are direct', () => {
        expect(roomMediaPath(['a', 'b', 'c'], { a: 'direct', b: 'direct', c: 'forwarded' })).toEqual({
            kind: 'mixed',
            direct: 2,
            total: 3,
        });
        expect(roomMediaPath(['a', 'b'], { a: 'direct' })).toEqual({ kind: 'mixed', direct: 1, total: 2 });
    });

    test('only the remote participants count, not stale entries', () => {
        expect(roomMediaPath(['a'], { a: 'forwarded', left: 'direct' })).toEqual({ kind: 'via_server' });
    });
});
