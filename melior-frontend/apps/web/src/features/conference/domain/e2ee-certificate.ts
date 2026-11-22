import { fromBase64Url, MAX_CERTIFICATE_CHARS, toBase64Url } from './e2ee-envelope';
import { isObject, isString } from './parse-primitives';

/**
 * Keycloak vouches for a device key. A browser asks the realm for an access token bound to its device key with
 * DPoP (RFC 9449): the token's `cnf.jkt` is the key's thumbprint, its `preferred_username` the person, and the
 * realm signs it. A peer that checks the signature against the realm's published keys knows that this device
 * belongs to that person: the Melior server, which only relays the token, cannot forge one. The token grants
 * nothing (its client has no roles), and without the device's private key it cannot even be presented.
 */

/** Where the realm is, and the client whose tokens vouch for device keys. Served with the app. */
export type VouchConfig = { issuer: string; clientId: string };

export const parseVouchConfig = (value: unknown): VouchConfig | null => {
    if (!isObject(value) || !isString(value.issuer) || !isString(value.clientId)) return null;
    if (!/^https?:\/\//.test(value.issuer) || value.clientId.length === 0) return null;
    return { issuer: value.issuer.replace(/\/+$/, ''), clientId: value.clientId };
};

/** A signed token split into what the checks need. Nothing in it is trusted before the signature verified. */
export type Certificate = {
    header: { alg: string; kid: string };
    claims: Record<string, unknown>;
    /** `header.payload`, the bytes the signature covers. */
    signedPart: Uint8Array<ArrayBuffer>;
    signature: Uint8Array<ArrayBuffer>;
};

const decodeJson = (part: string): unknown => {
    const bytes = fromBase64Url(part);
    if (!bytes) return null;
    try {
        return JSON.parse(new TextDecoder().decode(bytes));
    } catch {
        return null;
    }
};

/** Splits a compact JWS. Null for anything that is not three base64url parts with a JSON header and payload. */
export const parseCertificate = (token: string): Certificate | null => {
    if (token.length > MAX_CERTIFICATE_CHARS) return null;
    const parts = token.split('.');
    if (parts.length !== 3) return null;
    const [headerPart = '', payloadPart = '', signaturePart = ''] = parts;
    const header = decodeJson(headerPart);
    const claims = decodeJson(payloadPart);
    const signature = fromBase64Url(signaturePart);
    if (!isObject(header) || !isObject(claims) || !signature) return null;
    if (!isString(header.alg) || !isString(header.kid)) return null;
    return {
        header: { alg: header.alg, kid: header.kid },
        claims,
        signedPart: new TextEncoder().encode(`${headerPart}.${payloadPart}`),
        signature,
    };
};

/**
 * The RFC 7638 thumbprint input of an uncompressed P-256 public key (65 bytes): the required members in
 * lexicographic order, without spaces. Its SHA-256, base64url-encoded, is what `cnf.jkt` holds.
 */
export const thumbprintInput = (publicRaw: Uint8Array): string => {
    const x = toBase64Url(publicRaw.subarray(1, 33));
    const y = toBase64Url(publicRaw.subarray(33, 65));
    return `{"crv":"P-256","kty":"EC","x":"${x}","y":"${y}"}`;
};

/** The public JWK of an uncompressed P-256 key, as a DPoP proof's header carries it. */
export const publicJwk = (publicRaw: Uint8Array) => ({
    kty: 'EC',
    crv: 'P-256',
    x: toBase64Url(publicRaw.subarray(1, 33)),
    y: toBase64Url(publicRaw.subarray(33, 65)),
});

/** Why a certificate does not vouch for a device; `null` when every claim fits. */
export type ClaimProblem = 'issuer' | 'client' | 'expired' | 'username' | 'binding';

/**
 * Checks a certificate's claims, once its signature verified: issued by the configured realm, for the vouching
 * client, not expired, naming a person, and bound to the device key whose thumbprint is `jkt`. Returns the
 * username the realm vouches for.
 */
export const checkClaims = (
    claims: Record<string, unknown>,
    config: VouchConfig,
    jkt: string,
    nowSeconds: number,
): { username: string } | { problem: ClaimProblem } => {
    if (claims.iss !== config.issuer) return { problem: 'issuer' };
    if (claims.azp !== config.clientId) return { problem: 'client' };
    if (typeof claims.exp !== 'number' || claims.exp <= nowSeconds) return { problem: 'expired' };
    if (!isString(claims.preferred_username) || claims.preferred_username.length === 0) return { problem: 'username' };
    const cnf = claims.cnf;
    if (!isObject(cnf) || cnf.jkt !== jkt) return { problem: 'binding' };
    return { username: claims.preferred_username };
};

/** A certificate is renewed when less than this is left of it, so it outlasts the call it is shown in. */
export const CERTIFICATE_RENEW_SECONDS = 3600;

/** After a renewal that brought no certificate (no realm session, the realm down), wait this long before the next. */
export const VOUCH_RETRY_MS = 10 * 60_000;

/**
 * What to do about this browser's own certificate: `use` the one it has, `renew` it with a round trip to the
 * realm, or `wait` because a renewal failed a moment ago (a loop of redirects would lock the person out). A
 * renewal is a full-page redirect, so it only ever happens before a call starts, never during one.
 */
export type VouchAction = 'use' | 'renew' | 'wait';

export const vouchAction = (expSeconds: number | null, lastAttemptMs: number | null, nowMs: number): VouchAction => {
    if (expSeconds !== null && expSeconds * 1000 - nowMs > CERTIFICATE_RENEW_SECONDS * 1000) return 'use';
    if (lastAttemptMs !== null && nowMs - lastAttemptMs >= 0 && nowMs - lastAttemptMs < VOUCH_RETRY_MS) return 'wait';
    return 'renew';
};

/** Whether a certificate can still be shown at all: it has not expired. */
export const certificateUsable = (expSeconds: number | null, nowMs: number): boolean =>
    expSeconds !== null && expSeconds * 1000 > nowMs;

/** The `exp` of a certificate's claims, or null. Only the own, already trusted certificate is read this way. */
export const certificateExpiry = (claims: Record<string, unknown>): number | null =>
    typeof claims.exp === 'number' && Number.isFinite(claims.exp) ? claims.exp : null;

/**
 * Where the browser goes back to after the round trip: an app-relative path, never another site. Anything else
 * (`//host`, `/\\host`, an absolute URL, nothing) means the start page.
 */
export const safeReturnPath = (raw: string | null): string => {
    if (raw === null || !raw.startsWith('/')) return '/';
    if (raw.startsWith('//') || raw.startsWith('/\\')) return '/';
    return raw;
};
