import { afterEach, describe, expect, test, vi } from 'vitest';
import { toBase64Url } from '@/features/conference/domain';
import { createDeviceKey } from './e2ee-crypto';
import { createVoucher, deviceThumbprint } from './e2ee-vouch';

const ISSUER = 'http://localhost:8300/realms/melior';
const encode = (value: object) => toBase64Url(new TextEncoder().encode(JSON.stringify(value)));

/** A realm signing key, and a fetch that serves the app's realm settings and the realm's published keys. */
const realm = async () => {
    const pair = await crypto.subtle.generateKey(
        { name: 'RSASSA-PKCS1-v1_5', modulusLength: 2048, publicExponent: new Uint8Array([1, 0, 1]), hash: 'SHA-256' },
        true,
        ['sign', 'verify'],
    );
    const jwk = { ...(await crypto.subtle.exportKey('jwk', pair.publicKey)), kid: 'k1', alg: 'RS256' };
    vi.stubGlobal(
        'fetch',
        vi.fn(async (url: string) => {
            if (url === '/e2ee.json') return new Response(JSON.stringify({ issuer: ISSUER, clientId: 'melior-e2ee' }));
            if (url === `${ISSUER}/protocol/openid-connect/certs`) return new Response(JSON.stringify({ keys: [jwk] }));
            return new Response('', { status: 404 });
        }),
    );
    const sign = async (claims: object, kid = 'k1') => {
        const input = `${encode({ alg: 'RS256', kid })}.${encode(claims)}`;
        const signature = await crypto.subtle.sign(
            'RSASSA-PKCS1-v1_5',
            pair.privateKey,
            new TextEncoder().encode(input),
        );
        return `${input}.${toBase64Url(new Uint8Array(signature))}`;
    };
    return { sign };
};

const claimsFor = async (device: Uint8Array, username: string) => ({
    iss: ISSUER,
    azp: 'melior-e2ee',
    exp: Math.floor(Date.now() / 1000) + 3600,
    preferred_username: username,
    cnf: { jkt: await deviceThumbprint(device) },
});

describe('vouching for a device', () => {
    afterEach(() => {
        vi.unstubAllGlobals();
    });

    test('the thumbprint is the RFC 7638 thumbprint of the device key', async () => {
        const pair = await crypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign']);
        const jwk = await crypto.subtle.exportKey('jwk', pair.publicKey);
        const raw = new Uint8Array(await crypto.subtle.exportKey('raw', pair.publicKey));
        const canonical = JSON.stringify({ crv: jwk.crv, kty: jwk.kty, x: jwk.x, y: jwk.y });
        const expected = toBase64Url(
            new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(canonical))),
        );
        expect(await deviceThumbprint(raw)).toBe(expected);
    });

    test('a token the realm signed for this device and this name verifies', async () => {
        const { sign } = await realm();
        const device = await createDeviceKey();
        const voucher = await createVoucher(device);
        expect(voucher.vouching).toBe(true);
        const token = await sign(await claimsFor(device.publicRaw, 'bob'));
        expect(await voucher.verify(token, device.publicRaw, 'bob')).toBe('verified');
    });

    test('another device, another name, a foreign key or a forged signature is invalid; no token is unverified', async () => {
        const { sign } = await realm();
        const device = await createDeviceKey();
        const other = await createDeviceKey();
        const voucher = await createVoucher(device);
        const token = await sign(await claimsFor(device.publicRaw, 'bob'));
        expect(await voucher.verify(token, other.publicRaw, 'bob')).toBe('invalid');
        expect(await voucher.verify(token, device.publicRaw, 'alice')).toBe('invalid');
        expect(
            await voucher.verify(await sign(await claimsFor(device.publicRaw, 'bob'), 'k2'), device.publicRaw, 'bob'),
        ).toBe('invalid');
        const [header, payload] = token.split('.');
        const forged = `${header}.${payload}.${toBase64Url(new Uint8Array(256))}`;
        expect(await voucher.verify(forged, device.publicRaw, 'bob')).toBe('invalid');
        expect(await voucher.verify(null, device.publicRaw, 'bob')).toBe('unverified');
    });

    test('without realm settings nothing is vouched for, and keys still flow', async () => {
        vi.stubGlobal(
            'fetch',
            vi.fn(async () => new Response('', { status: 404 })),
        );
        const voucher = await createVoucher(await createDeviceKey());
        expect(voucher.vouching).toBe(false);
        expect(await voucher.ownCertificate()).toBeNull();
    });
});
