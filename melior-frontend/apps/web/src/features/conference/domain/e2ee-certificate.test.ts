import { describe, expect, test } from 'vitest';
import {
    CERTIFICATE_RENEW_SECONDS,
    certificateExpiry,
    certificateUsable,
    checkClaims,
    parseCertificate,
    parseVouchConfig,
    safeReturnPath,
    thumbprintInput,
    VOUCH_RETRY_MS,
    vouchAction,
} from './e2ee-certificate';
import { toBase64Url } from './e2ee-envelope';

const part = (value: unknown) => toBase64Url(new TextEncoder().encode(JSON.stringify(value)));
const config = { issuer: 'http://localhost:8300/realms/melior', clientId: 'melior-e2ee' };
const claims = (over: Record<string, unknown> = {}) => ({
    iss: config.issuer,
    azp: 'melior-e2ee',
    exp: 2000,
    preferred_username: 'alice',
    cnf: { jkt: 'thumb' },
    ...over,
});

describe('vouch config', () => {
    test('takes an http(s) issuer and a client id, without a trailing slash', () => {
        expect(parseVouchConfig({ issuer: 'https://id.example/realms/m/', clientId: 'c' })).toEqual({
            issuer: 'https://id.example/realms/m',
            clientId: 'c',
        });
    });

    test('anything else means no vouching', () => {
        for (const bad of [null, {}, { issuer: 'ftp://x', clientId: 'c' }, { issuer: 'https://x', clientId: '' }]) {
            expect(parseVouchConfig(bad)).toBeNull();
        }
    });
});

describe('certificate parsing', () => {
    test('splits a compact JWS', () => {
        const token = `${part({ alg: 'RS256', kid: 'k1' })}.${part({ a: 1 })}.${toBase64Url(new Uint8Array([1, 2]))}`;
        const parsed = parseCertificate(token);
        expect(parsed?.header).toEqual({ alg: 'RS256', kid: 'k1' });
        expect(parsed?.claims).toEqual({ a: 1 });
        expect(parsed?.signature).toEqual(new Uint8Array([1, 2]));
    });

    test('rejects malformed tokens', () => {
        for (const bad of ['', 'a.b', 'a.b.c.d', `${part({ alg: 'RS256' })}.${part({})}.AA`, `!.${part({})}.AA`]) {
            expect(parseCertificate(bad)).toBeNull();
        }
        expect(parseCertificate('a'.repeat(5000))).toBeNull();
    });
});

describe('certificate claims', () => {
    test('a token of the realm, for the client, current, bound to the key, vouches for its username', () => {
        expect(checkClaims(claims(), config, 'thumb', 1000)).toEqual({ username: 'alice' });
    });

    test('names what does not fit', () => {
        expect(checkClaims(claims({ iss: 'https://evil' }), config, 'thumb', 1000)).toEqual({ problem: 'issuer' });
        expect(checkClaims(claims({ azp: 'melior' }), config, 'thumb', 1000)).toEqual({ problem: 'client' });
        expect(checkClaims(claims(), config, 'thumb', 2000)).toEqual({ problem: 'expired' });
        expect(checkClaims(claims({ preferred_username: '' }), config, 'thumb', 1000)).toEqual({ problem: 'username' });
        expect(checkClaims(claims(), config, 'other', 1000)).toEqual({ problem: 'binding' });
        expect(checkClaims(claims({ cnf: undefined }), config, 'thumb', 1000)).toEqual({ problem: 'binding' });
    });

    test('an own certificate is used until its last hour, then renewed, unless a renewal just failed', () => {
        const now = 1_000_000;
        const exp = (left: number) => now / 1000 + left;
        expect(vouchAction(exp(CERTIFICATE_RENEW_SECONDS + 1), null, now)).toBe('use');
        expect(vouchAction(exp(CERTIFICATE_RENEW_SECONDS), null, now)).toBe('renew');
        expect(vouchAction(null, null, now)).toBe('renew');
        expect(vouchAction(null, now - 1000, now)).toBe('wait');
        expect(vouchAction(null, now - VOUCH_RETRY_MS, now)).toBe('renew');
        // A clock that went back does not block renewal.
        expect(vouchAction(null, now + 5000, now)).toBe('renew');
    });

    test('a certificate is shown while it has not expired', () => {
        expect(certificateUsable(1001, 1_000_000)).toBe(true);
        expect(certificateUsable(1000, 1_000_000)).toBe(false);
        expect(certificateUsable(null, 0)).toBe(false);
        expect(certificateExpiry({ exp: 5 })).toBe(5);
        expect(certificateExpiry({ exp: '5' })).toBeNull();
    });
});

describe('thumbprint input', () => {
    test('is the canonical RFC 7638 member order of an EC key', () => {
        const raw = Uint8Array.from({ length: 65 }, (_, i) => i);
        const input = thumbprintInput(raw);
        expect(input.startsWith('{"crv":"P-256","kty":"EC","x":"')).toBe(true);
        expect(JSON.parse(input)).toEqual({
            crv: 'P-256',
            kty: 'EC',
            x: toBase64Url(raw.subarray(1, 33)),
            y: toBase64Url(raw.subarray(33, 65)),
        });
    });
});

describe('return path', () => {
    test('keeps an app-relative path and refuses everything else', () => {
        expect(safeReturnPath('/c/1/2?relay=true#x')).toBe('/c/1/2?relay=true#x');
        for (const bad of [null, '', 'https://evil.example', '//evil.example', '/\\evil.example', 'c/1']) {
            expect(safeReturnPath(bad)).toBe('/');
        }
    });
});
