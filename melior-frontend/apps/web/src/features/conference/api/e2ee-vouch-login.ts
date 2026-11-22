import {
    certificateExpiry,
    certificateUsable,
    parseCertificate,
    publicJwk,
    safeReturnPath,
    toBase64Url,
    type VouchConfig,
    vouchAction,
} from '@/features/conference/domain';
import { type DeviceKey, randomText, sha256 } from './e2ee-crypto';

/**
 * This browser's own certificate: a full-page round trip to the realm (`prompt=none`, so nothing is shown when the
 * person's Keycloak session is alive), back to `/e2ee-callback`, where the code is traded for an access token
 * bound to the device key with DPoP (RFC 9449).
 *
 * A top-level navigation sends Keycloak its own session cookie as a first-party cookie, so this works when the
 * browser blocks third-party cookies, and when Keycloak runs on another site than the app. A hidden frame would
 * not. The round trip happens only before a call starts, never during one.
 */

/** The app route the realm sends the browser back to. Must be a redirect URI of the vouching client. */
export const CALLBACK_PATH = '/e2ee-callback';

/** Kept across tabs: the token is useless without the device key, which no script can read. */
const TOKEN_KEY = 'melior.e2ee.certificate';
const ATTEMPT_KEY = 'melior.e2ee.attempt';
/** Kept in this tab only, for the length of one round trip. */
const PENDING_KEY = 'melior.e2ee.pending';

type StoredCertificate = { token: string; thumbprint: string };
type Pending = { state: string; verifier: string; back: string };

/** Storage that may be missing or refuse access (private modes); every failure reads as "nothing stored". */
const read = (storage: () => Storage, key: string): unknown => {
    try {
        const raw = storage().getItem(key);
        return raw === null ? null : JSON.parse(raw);
    } catch {
        return null;
    }
};

const write = (storage: () => Storage, key: string, value: unknown | null): void => {
    try {
        if (value === null) storage().removeItem(key);
        else storage().setItem(key, JSON.stringify(value));
    } catch {
        // Without storage, the certificate lasts for this page only; the next call asks again.
    }
};

const local = () => window.localStorage;
const session = () => window.sessionStorage;

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null;

const storedToken = (thumbprint: string): string | null => {
    const value = read(local, TOKEN_KEY);
    if (!isRecord(value) || typeof value.token !== 'string' || value.thumbprint !== thumbprint) return null;
    return value.token;
};

const expiryOf = (token: string | null): number | null => {
    const claims = token ? parseCertificate(token)?.claims : null;
    return claims ? certificateExpiry(claims) : null;
};

/** The token for this device key, while it has not expired; null otherwise. */
export const storedCertificate = (thumbprint: string): string | null => {
    const token = storedToken(thumbprint);
    return certificateUsable(expiryOf(token), Date.now()) ? token : null;
};

const authorizeUrl = (config: VouchConfig, state: string, challenge: string): string => {
    const url = new URL(`${config.issuer}/protocol/openid-connect/auth`);
    const query = {
        client_id: config.clientId,
        redirect_uri: `${window.location.origin}${CALLBACK_PATH}`,
        response_type: 'code',
        scope: 'openid',
        prompt: 'none',
        state,
        code_challenge: challenge,
        code_challenge_method: 'S256',
    };
    for (const [name, value] of Object.entries(query)) url.searchParams.set(name, value);
    return url.toString();
};

/** Set once this page started a round trip: a second start (an effect run twice) must not begin another one. */
let navigating = false;

/**
 * Starts the round trip when the token is missing or in its last hour, unless one failed a moment ago. Returns
 * true when the page is navigating away, also to every caller after the first.
 */
export const renewCertificate = async (config: VouchConfig, thumbprint: string, back: string): Promise<boolean> => {
    if (navigating) return true;
    const attempt = read(local, ATTEMPT_KEY);
    const action = vouchAction(
        expiryOf(storedToken(thumbprint)),
        typeof attempt === 'number' ? attempt : null,
        Date.now(),
    );
    if (action !== 'renew') return false;
    navigating = true;
    const pending: Pending = { state: randomText(16), verifier: randomText(32), back: safeReturnPath(back) };
    write(local, ATTEMPT_KEY, Date.now());
    write(session, PENDING_KEY, pending);
    window.location.assign(authorizeUrl(config, pending.state, toBase64Url(await sha256(pending.verifier))));
    return true;
};

const nowSeconds = () => Math.floor(Date.now() / 1000);

const signJwt = async (device: DeviceKey, header: object, payload: object): Promise<string> => {
    const encode = (value: object) => toBase64Url(new TextEncoder().encode(JSON.stringify(value)));
    const signingInput = `${encode(header)}.${encode(payload)}`;
    const signature = await crypto.subtle.sign(
        { name: 'ECDSA', hash: 'SHA-256' },
        device.privateKey,
        new TextEncoder().encode(signingInput),
    );
    return `${signingInput}.${toBase64Url(new Uint8Array(signature))}`;
};

/** A DPoP proof (RFC 9449, 4.2) for one request, signed by the device key. */
const dpopProof = (device: DeviceKey, url: string, nonce: string | null): Promise<string> =>
    signJwt(
        device,
        { typ: 'dpop+jwt', alg: 'ES256', jwk: publicJwk(device.publicRaw) },
        { jti: randomText(16), htm: 'POST', htu: url, iat: nowSeconds(), ...(nonce ? { nonce } : {}) },
    );

/** Trades the code for a DPoP-bound token, answering one `use_dpop_nonce` challenge if the realm sends it. */
const redeem = async (config: VouchConfig, device: DeviceKey, code: string, verifier: string) => {
    const tokenUrl = `${config.issuer}/protocol/openid-connect/token`;
    const body = new URLSearchParams({
        grant_type: 'authorization_code',
        client_id: config.clientId,
        code,
        code_verifier: verifier,
        redirect_uri: `${window.location.origin}${CALLBACK_PATH}`,
    });
    const post = async (nonce: string | null) =>
        fetch(tokenUrl, {
            method: 'POST',
            headers: {
                'content-type': 'application/x-www-form-urlencoded',
                DPoP: await dpopProof(device, tokenUrl, nonce),
            },
            body,
        });
    let response = await post(null);
    const nonce = response.headers.get('DPoP-Nonce');
    if (!response.ok && nonce) response = await post(nonce);
    if (!response.ok) return null;
    const token = (await response.json()) as { access_token?: unknown };
    return typeof token.access_token === 'string' ? token.access_token : null;
};

const readPending = (): Pending | null => {
    const value = read(session, PENDING_KEY);
    write(session, PENDING_KEY, null);
    if (!isRecord(value)) return null;
    const { state, verifier, back } = value;
    if (typeof state !== 'string' || typeof verifier !== 'string' || typeof back !== 'string') return null;
    return { state, verifier, back };
};

/**
 * The end of the round trip, on `/e2ee-callback`: checks `state`, trades the code and keeps the token. Returns
 * where to go back to. A failed or foreign round trip keeps nothing; the device then counts as unverified until
 * the next renewal is due.
 */
export const finishRenewal = async (
    config: VouchConfig | null,
    device: DeviceKey,
    thumbprint: string,
    search: string,
): Promise<string> => {
    const pending = readPending();
    const query = new URLSearchParams(search);
    const code = query.get('code');
    if (!config || !pending || !code || query.get('state') !== pending.state)
        return safeReturnPath(pending?.back ?? null);
    try {
        const token = await redeem(config, device, code, pending.verifier);
        if (token) {
            write(local, TOKEN_KEY, { token, thumbprint } satisfies StoredCertificate);
            write(local, ATTEMPT_KEY, null);
        }
    } catch (error) {
        console.warn('the realm could not vouch for this device', error);
    }
    return safeReturnPath(pending.back);
};
