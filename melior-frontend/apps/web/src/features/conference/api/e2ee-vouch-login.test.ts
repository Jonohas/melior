import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { toBase64Url } from '@/features/conference/domain';
import { createDeviceKey } from './e2ee-crypto';
import { CALLBACK_PATH } from './e2ee-vouch-login';

/** A fresh page load: the module's once-per-page state starts over. */
const page = async () => {
    vi.resetModules();
    const { finishRenewal, renewCertificate, storedCertificate } = await import('./e2ee-vouch-login');
    return { finishRenewal, renewCertificate, storedCertificate };
};

const CONFIG = { issuer: 'https://auth.example.org/realms/melior', clientId: 'melior-e2ee' };

const memoryStorage = (): Storage => {
    const items = new Map<string, string>();
    return {
        getItem: (key) => items.get(key) ?? null,
        setItem: (key, value) => void items.set(key, value),
        removeItem: (key) => void items.delete(key),
        clear: () => items.clear(),
        key: () => null,
        get length() {
            return items.size;
        },
    };
};

const tokenExpiringIn = (seconds: number) => {
    const part = (value: object) => toBase64Url(new TextEncoder().encode(JSON.stringify(value)));
    return `${part({ alg: 'RS256', kid: 'k' })}.${part({ exp: Math.floor(Date.now() / 1000) + seconds })}.AA`;
};

describe('the realm round trip', () => {
    let assigned: string[];

    beforeEach(() => {
        assigned = [];
        vi.stubGlobal('window', {
            localStorage: memoryStorage(),
            sessionStorage: memoryStorage(),
            location: { origin: 'https://melior.example.org', assign: (url: string) => assigned.push(url) },
        });
    });

    afterEach(() => {
        vi.unstubAllGlobals();
    });

    test('without a token it navigates to the realm with prompt=none, PKCE and the app callback', async () => {
        const { renewCertificate } = await page();
        expect(await renewCertificate(CONFIG, 'thumb', '/c/1/2')).toBe(true);
        const url = new URL(assigned[0] ?? '');
        expect(url.origin + url.pathname).toBe(`${CONFIG.issuer}/protocol/openid-connect/auth`);
        expect(url.searchParams.get('prompt')).toBe('none');
        expect(url.searchParams.get('code_challenge_method')).toBe('S256');
        expect(url.searchParams.get('redirect_uri')).toBe(`https://melior.example.org${CALLBACK_PATH}`);
    });

    test('a second start on the same page does not begin another round trip', async () => {
        const { renewCertificate } = await page();
        expect(await renewCertificate(CONFIG, 'thumb', '/')).toBe(true);
        expect(await renewCertificate(CONFIG, 'thumb', '/')).toBe(true);
        expect(assigned).toHaveLength(1);
    });

    test('after a round trip that brought nothing, the next page does not navigate again at once: no loop', async () => {
        expect(await (await page()).renewCertificate(CONFIG, 'thumb', '/')).toBe(true);
        const callback = await page();
        expect(await callback.finishRenewal(CONFIG, await createDeviceKey(), 'thumb', '?error=login_required')).toBe(
            '/',
        );
        expect(await (await page()).renewCertificate(CONFIG, 'thumb', '/')).toBe(false);
        expect(assigned).toHaveLength(1);
    });

    test('a token kept for this device is used; one kept for another device key is not', async () => {
        const { renewCertificate, storedCertificate } = await page();
        const token = tokenExpiringIn(12 * 3600);
        window.localStorage.setItem('melior.e2ee.certificate', JSON.stringify({ token, thumbprint: 'thumb' }));
        expect(storedCertificate('thumb')).toBe(token);
        expect(storedCertificate('other')).toBeNull();
        expect(await renewCertificate(CONFIG, 'thumb', '/')).toBe(false);
    });

    test('a token in its last hour is renewed; an expired one is not shown', async () => {
        const { renewCertificate, storedCertificate } = await page();
        const token = tokenExpiringIn(600);
        window.localStorage.setItem('melior.e2ee.certificate', JSON.stringify({ token, thumbprint: 'thumb' }));
        expect(storedCertificate('thumb')).toBe(token);
        expect(await renewCertificate(CONFIG, 'thumb', '/')).toBe(true);
        window.localStorage.setItem(
            'melior.e2ee.certificate',
            JSON.stringify({ token: tokenExpiringIn(-1), thumbprint: 'thumb' }),
        );
        expect(storedCertificate('thumb')).toBeNull();
    });

    test('a callback with a foreign state keeps nothing and goes back to a safe path only', async () => {
        const fetch = vi.fn();
        vi.stubGlobal('fetch', fetch);
        await (await page()).renewCertificate(CONFIG, 'thumb', '//evil.example');
        const { finishRenewal, storedCertificate } = await page();
        const back = await finishRenewal(CONFIG, await createDeviceKey(), 'thumb', '?code=abc&state=not-ours');
        expect(back).toBe('/');
        expect(fetch).not.toHaveBeenCalled();
        expect(storedCertificate('thumb')).toBeNull();
    });

    test('a matching callback trades the code with a DPoP proof and keeps the token', async () => {
        const token = tokenExpiringIn(12 * 3600);
        const fetch = vi.fn(async (_url: string, init: RequestInit) => {
            const proof = (init.headers as Record<string, string>).DPoP ?? '';
            expect(proof.split('.')).toHaveLength(3);
            return new Response(JSON.stringify({ access_token: token }));
        });
        vi.stubGlobal('fetch', fetch);
        await (await page()).renewCertificate(CONFIG, 'thumb', '/c/1/2');
        const { finishRenewal, storedCertificate } = await page();
        const state = new URL(assigned[0] ?? '').searchParams.get('state');
        const back = await finishRenewal(CONFIG, await createDeviceKey(), 'thumb', `?code=abc&state=${state}`);
        expect(back).toBe('/c/1/2');
        expect(fetch).toHaveBeenCalledWith(`${CONFIG.issuer}/protocol/openid-connect/token`, expect.anything());
        expect(storedCertificate('thumb')).toBe(token);
    });
});
