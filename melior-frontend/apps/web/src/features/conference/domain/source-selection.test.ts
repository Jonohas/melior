import { describe, expect, test } from 'vitest';
import type { MediaKind } from './path-flow';
import {
    applyRoute,
    initialRouteView,
    type PathObservation,
    type PeerRouteView,
    selectSource,
} from './source-selection';

const obs = (connected: boolean, flowing: MediaKind[]): PathObservation => ({ connected, flowing });
const toDirect: PeerRouteView = applyRoute(initialRouteView(), { path: 'direct', epoch: 1 });
const both = ['audio', 'video'] as MediaKind[];

describe('selectSource', () => {
    test('switches to the preferred path once it carries what the shown path carries, and reports once', () => {
        const paths = { forwarded: obs(true, both), direct: obs(true, both) };
        const first = selectSource(toDirect, paths);
        expect(first.view.shown).toBe('direct');
        expect(first.report).toBe(1);
        const again = selectSource(first.view, paths);
        expect(again.report).toBeNull();
        expect(again.view.shown).toBe('direct');
    });

    test('waits while the preferred path is not connected or carries too little', () => {
        const notConnected = selectSource(toDirect, { forwarded: obs(true, both), direct: obs(false, both) });
        expect(notConnected).toEqual({ view: toDirect, report: null });
        const audioOnly = selectSource(toDirect, { forwarded: obs(true, both), direct: obs(true, ['audio']) });
        expect(audioOnly).toEqual({ view: toDirect, report: null });
        const idle = selectSource(toDirect, { forwarded: obs(true, both), direct: obs(true, []) });
        expect(idle).toEqual({ view: toDirect, report: null });
    });

    test('does not wait for a kind that sends nothing (camera off)', () => {
        const result = selectSource(toDirect, { forwarded: obs(true, ['audio']), direct: obs(true, ['audio']) });
        expect(result.view.shown).toBe('direct');
        expect(result.report).toBe(1);
    });

    test('falls back to the other path without reporting when the shown path died', () => {
        const shownDirect = { ...toDirect, shown: 'direct' as const, preferred: 'forwarded' as const, epoch: 2 };
        const result = selectSource(shownDirect, { direct: obs(false, []), forwarded: obs(false, both) });
        expect(result.view.shown).toBe('forwarded');
        expect(result.report).toBeNull();
    });

    test('keeps the shown path when nothing flows anywhere', () => {
        const result = selectSource(toDirect, { forwarded: obs(true, []), direct: obs(true, []) });
        expect(result).toEqual({ view: toDirect, report: null });
    });

    test('a peer already on the preferred path reports the new epoch once', () => {
        const back = applyRoute(initialRouteView(), { path: 'forwarded', epoch: 3 });
        const paths = { forwarded: obs(true, both), direct: obs(false, []) };
        const first = selectSource(back, paths);
        expect(first.report).toBe(3);
        expect(selectSource(first.view, paths).report).toBeNull();
    });

    test('never reports before any route arrived', () => {
        const paths = { forwarded: obs(true, both), direct: obs(false, []) };
        expect(selectSource(initialRouteView(), paths).report).toBeNull();
    });

    test('a new epoch is reported again after the previous one', () => {
        const paths = { forwarded: obs(true, both), direct: obs(true, both) };
        const shownDirect = selectSource(toDirect, paths).view;
        const back = applyRoute(shownDirect, { path: 'forwarded', epoch: 2 });
        const result = selectSource(back, paths);
        expect(result.view.shown).toBe('forwarded');
        expect(result.report).toBe(2);
    });
});

describe('applyRoute', () => {
    test('ignores stale and repeated epochs', () => {
        const current = applyRoute(initialRouteView(), { path: 'direct', epoch: 5 });
        expect(applyRoute(current, { path: 'forwarded', epoch: 4 })).toBe(current);
        expect(applyRoute(current, { path: 'forwarded', epoch: 5 })).toBe(current);
        expect(applyRoute(current, { path: 'forwarded', epoch: 6 }).preferred).toBe('forwarded');
    });
});
