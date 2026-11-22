import { describe, expect, test } from 'vitest';
import { inferLinkRoute } from './debug-link-route';
import { initialRouteView, type PeerRouteView } from './source-selection';

const view = (change: Partial<PeerRouteView>): PeerRouteView => ({ ...initialRouteView(), ...change });

describe('inferLinkRoute', () => {
    test('a new link is forwarded', () => {
        expect(inferLinkRoute({ view: initialRouteView(), linkOpen: false, sfuCarriesPeer: true })).toBe('Forwarded');
    });

    test('a route to direct is ToDirect until this side is live and the SFU stopped sending the peer', () => {
        const toDirect = view({ preferred: 'direct', epoch: 2 });
        expect(inferLinkRoute({ view: toDirect, linkOpen: true, sfuCarriesPeer: true })).toBe('ToDirect');
        const live = { ...toDirect, reportedEpoch: 2, shown: 'direct' as const };
        expect(inferLinkRoute({ view: live, linkOpen: true, sfuCarriesPeer: true })).toBe('ToDirect');
        expect(inferLinkRoute({ view: live, linkOpen: true, sfuCarriesPeer: false })).toBe('Direct');
    });

    test('a report for an older epoch does not make the link direct', () => {
        const stale = view({ preferred: 'direct', epoch: 4, reportedEpoch: 2 });
        expect(inferLinkRoute({ view: stale, linkOpen: true, sfuCarriesPeer: false })).toBe('ToDirect');
    });

    test('a route back to forwarded is ToForwarded while the direct link is still open', () => {
        const back = view({ preferred: 'forwarded', epoch: 3 });
        expect(inferLinkRoute({ view: back, linkOpen: true, sfuCarriesPeer: true })).toBe('ToForwarded');
        expect(inferLinkRoute({ view: back, linkOpen: false, sfuCarriesPeer: true })).toBe('Forwarded');
    });
});
