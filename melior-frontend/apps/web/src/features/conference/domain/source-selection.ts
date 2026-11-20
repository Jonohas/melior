import type { MediaKind } from './path-flow';
import type { Path } from './signalling-message';

/** What the client knows about one remote peer's route and which path it currently plays. */
export type PeerRouteView = {
    /** From the last `route`; `forwarded` before any arrives. */
    preferred: Path;
    /** Epoch of the last accepted `route`; null before any. */
    epoch: number | null;
    /** The epoch for which `route_live` was already sent. */
    reportedEpoch: number | null;
    shown: Path;
};

export type PathObservation = {
    connected: boolean;
    flowing: readonly MediaKind[];
};

export type SelectionResult = {
    view: PeerRouteView;
    /** The epoch to report with `route_live`, at most once per epoch. */
    report: number | null;
};

/** What a path shows when nothing is known about it. */
export const NO_OBSERVATION: PathObservation = { connected: false, flowing: [] };

export const initialRouteView = (): PeerRouteView => ({
    preferred: 'forwarded',
    epoch: null,
    reportedEpoch: null,
    shown: 'forwarded',
});

const otherPath = (path: Path): Path => (path === 'direct' ? 'forwarded' : 'direct');

/** A `route` with an epoch not newer than the current one is stale and changes nothing. */
export const applyRoute = (view: PeerRouteView, route: { path: Path; epoch: number }): PeerRouteView =>
    view.epoch !== null && route.epoch <= view.epoch ? view : { ...view, preferred: route.path, epoch: route.epoch };

const reportOnce = (view: PeerRouteView): SelectionResult => {
    if (view.epoch === null || view.reportedEpoch === view.epoch) return { view, report: null };
    return { view: { ...view, reportedEpoch: view.epoch }, report: view.epoch };
};

/** The shown path carries nothing but the other one does: it died (link closed or failed), so show the other. */
const recoverDeadPath = (view: PeerRouteView, paths: Record<Path, PathObservation>): PeerRouteView => {
    const other = otherPath(view.shown);
    const dead = paths[view.shown].flowing.length === 0 && paths[other].flowing.length > 0;
    return dead ? { ...view, shown: other } : view;
};

/**
 * Evaluated on every stats sample. Show the preferred path once its transport is connected and it carries
 * every kind the shown path carries (a kind that sends nothing, camera off, is not waited for) and at least
 * something; report `route_live` once per epoch. If the shown path died and the other one flows, fall back to
 * it without reporting, also when the shown path is the preferred one. A peer already on the preferred path
 * still reports its epoch once: the server waits for both sides in every transition, including one this
 * side never had to switch for.
 */
export const selectSource = (view: PeerRouteView, paths: Record<Path, PathObservation>): SelectionResult => {
    if (view.shown === view.preferred) {
        const reported = reportOnce(view);
        return { view: recoverDeadPath(reported.view, paths), report: reported.report };
    }
    const shown = paths[view.shown];
    const preferred = paths[view.preferred];
    const preferredCarriesShown = shown.flowing.every((kind) => preferred.flowing.includes(kind));
    if (preferred.connected && preferred.flowing.length > 0 && preferredCarriesShown) {
        return reportOnce({ ...view, shown: view.preferred });
    }
    return { view: recoverDeadPath(view, paths), report: null };
};
