// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { LOGIN_COMPLETED_COOKIE } from './bff-cookies';
import { MAX_ATTEMPTS } from './login-attempt';
import { beginLoginRedirect, buildLoginHref, currentLocationHref } from './login-gate';

const KEY = 'test';

const expireMarker = () => {
    document.cookie = `${LOGIN_COMPLETED_COOKIE}=; path=/; max-age=0`;
};

beforeEach(() => {
    sessionStorage.clear();
    expireMarker();
    window.history.replaceState(null, '', '/rooms/x');
});

afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
});

describe('buildLoginHref', () => {
    test('encodes the app-relative target', () => {
        expect(buildLoginHref('/rooms/a?relay=true#h')).toBe('/login?redirectTo=%2Frooms%2Fa%3Frelay%3Dtrue%23h');
        expect(buildLoginHref('/')).toBe('/login?redirectTo=%2F');
    });
});

describe('currentLocationHref', () => {
    test('is path, search and hash', () => {
        window.history.replaceState(null, '', '/rooms/x?relay=1#top');
        expect(currentLocationHref()).toBe('/rooms/x?relay=1#top');
    });
});

describe('beginLoginRedirect', () => {
    test('redirects to the login href for the given target', () => {
        expect(beginLoginRedirect(KEY, '/rooms/x')).toEqual({ kind: 'redirect', href: buildLoginHref('/rooms/x') });
    });

    test('defaults the target to the current location', () => {
        window.history.replaceState(null, '', '/rooms/y?relay=1');
        expect(beginLoginRedirect(KEY)).toEqual({ kind: 'redirect', href: buildLoginHref('/rooms/y?relay=1') });
    });

    test('refuses on the login page itself, but not on a lookalike path', () => {
        window.history.replaceState(null, '', '/login');
        expect(beginLoginRedirect(KEY, '/')).toEqual({ kind: 'refused', reason: 'already-on-login' });
        window.history.replaceState(null, '', '/login-help');
        expect(beginLoginRedirect(KEY, '/').kind).toBe('redirect');
    });

    test('refuses and consumes the marker cookie after a completed round trip', () => {
        document.cookie = `${LOGIN_COMPLETED_COOKIE}=1; path=/`;
        expect(beginLoginRedirect(KEY, '/')).toEqual({ kind: 'refused', reason: 'round-trip-completed' });
        expect(document.cookie).not.toContain(LOGIN_COMPLETED_COOKIE);
        expect(beginLoginRedirect(KEY, '/').kind).toBe('redirect');
    });

    test('bills one attempt per page load and refuses once the budget is spent', () => {
        // Each loop iteration is a new document, i.e. a new performance.timeOrigin.
        const origin = vi.spyOn(performance, 'timeOrigin', 'get');
        for (let page = 1; page <= MAX_ATTEMPTS; page++) {
            origin.mockReturnValue(page);
            expect(beginLoginRedirect(KEY, '/').kind).toBe('redirect');
        }
        origin.mockReturnValue(MAX_ATTEMPTS + 1);
        expect(beginLoginRedirect(KEY, '/')).toEqual({ kind: 'refused', reason: 'budget-exhausted' });
    });

    test('repeat requests from the same page cost nothing', () => {
        for (let i = 0; i < MAX_ATTEMPTS * 3; i++) expect(beginLoginRedirect(KEY, '/').kind).toBe('redirect');
        expect(JSON.parse(sessionStorage.getItem(`${KEY}.login-attempts`) ?? '[]')).toHaveLength(1);
    });

    test('preloads are not billed', () => {
        for (let i = 0; i < MAX_ATTEMPTS * 3; i++) {
            expect(beginLoginRedirect(KEY, '/', { preload: true }).kind).toBe('redirect');
        }
        expect(sessionStorage.getItem(`${KEY}.login-attempts`)).toBeNull();
    });

    test('attempts expire after the window', () => {
        vi.useFakeTimers();
        const origin = vi.spyOn(performance, 'timeOrigin', 'get');
        for (let page = 1; page <= MAX_ATTEMPTS; page++) {
            origin.mockReturnValue(page);
            beginLoginRedirect(KEY, '/');
        }
        origin.mockReturnValue(100);
        expect(beginLoginRedirect(KEY, '/').kind).toBe('refused');
        vi.advanceTimersByTime(61_000);
        expect(beginLoginRedirect(KEY, '/').kind).toBe('redirect');
    });

    test('keeps the budget under melior.login-attempts for the app key', () => {
        beginLoginRedirect('melior', '/');
        expect(sessionStorage.getItem('melior.login-attempts')).not.toBeNull();
    });

    test('ignores corrupt storage', () => {
        sessionStorage.setItem(`${KEY}.login-attempts`, '{not json');
        expect(beginLoginRedirect(KEY, '/').kind).toBe('redirect');
    });
});
