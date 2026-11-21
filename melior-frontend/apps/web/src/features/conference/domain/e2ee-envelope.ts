import { isObject, isString } from './parse-primitives';

/**
 * The end-to-end key messages participants send each other through the server's `e2ee` relay. The server sees
 * only a base64url string; everything in it comes from another browser and is parsed as hostile input.
 *
 * - `hello`: who I am for this call. My device key, my per-call ECDH key, the device key's signature over both
 *   session ids and the ECDH key, so the server can neither swap the ECDH key nor replay it to someone else, and
 *   the realm's token vouching that the device key is mine.
 * - `key`: my media key, AES-GCM-encrypted for you under the key we derived from our two ECDH keys.
 */

export const E2EE_VERSION = 1;

/** Raw uncompressed P-256 public keys are 65 bytes. */
const P256_PUBLIC_BYTES = 65;
/** An ECDSA P-256 signature in IEEE P1363 form. */
const P256_SIGNATURE_BYTES = 64;
const WRAP_IV_BYTES = 12;
/** A 16-byte media key plus the 16-byte GCM tag. */
const WRAPPED_KEY_BYTES = 32;

/** The longest certificate (a compact JWS from the realm) a hello may carry. */
export const MAX_CERTIFICATE_CHARS = 4096;

export type Hello = {
    type: 'hello';
    device: Uint8Array<ArrayBuffer>;
    ephemeral: Uint8Array<ArrayBuffer>;
    signature: Uint8Array<ArrayBuffer>;
    /** The realm's token vouching for `device`, if this browser has one. */
    certificate: string | null;
};

export type WrappedKey = {
    type: 'key';
    keyId: number;
    iv: Uint8Array<ArrayBuffer>;
    wrapped: Uint8Array<ArrayBuffer>;
};

export type KeyMessage = Hello | WrappedKey;

export const toBase64Url = (bytes: Uint8Array): string => {
    let binary = '';
    for (const byte of bytes) binary += String.fromCharCode(byte);
    return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
};

/** Null for anything that is not unpadded base64url. */
export const fromBase64Url = (text: string): Uint8Array<ArrayBuffer> | null => {
    if (!/^[A-Za-z0-9_-]*$/.test(text)) return null;
    try {
        const binary = atob(text.replace(/-/g, '+').replace(/_/g, '/'));
        return Uint8Array.from(binary, (char) => char.charCodeAt(0));
    } catch {
        return null;
    }
};

const bytesOf = (value: unknown, length: number): Uint8Array<ArrayBuffer> | null => {
    if (!isString(value)) return null;
    const bytes = fromBase64Url(value);
    return bytes?.length === length ? bytes : null;
};

const isKeyId = (value: unknown): value is number =>
    typeof value === 'number' && Number.isInteger(value) && value >= 0 && value <= 0xffffffff;

const parseHello = (value: Record<string, unknown>): Hello | null => {
    const device = bytesOf(value.d, P256_PUBLIC_BYTES);
    const ephemeral = bytesOf(value.e, P256_PUBLIC_BYTES);
    const signature = bytesOf(value.s, P256_SIGNATURE_BYTES);
    if (!device || !ephemeral || !signature) return null;
    const certificate = value.c === undefined ? null : value.c;
    if (certificate !== null && (!isString(certificate) || certificate.length > MAX_CERTIFICATE_CHARS)) return null;
    return { type: 'hello', device, ephemeral, signature, certificate };
};

const parseWrappedKey = (value: Record<string, unknown>): WrappedKey | null => {
    const iv = bytesOf(value.i, WRAP_IV_BYTES);
    const wrapped = bytesOf(value.w, WRAPPED_KEY_BYTES);
    if (!isKeyId(value.k) || !iv || !wrapped) return null;
    return { type: 'key', keyId: value.k, iv, wrapped };
};

/** Reads a relayed payload. Null for anything malformed, of another version or of an unknown type. */
export const parseKeyMessage = (payload: string): KeyMessage | null => {
    const bytes = fromBase64Url(payload);
    if (!bytes) return null;
    let value: unknown;
    try {
        value = JSON.parse(new TextDecoder().decode(bytes));
    } catch {
        return null;
    }
    if (!isObject(value) || value.v !== E2EE_VERSION) return null;
    if (value.t === 'hello') return parseHello(value);
    if (value.t === 'key') return parseWrappedKey(value);
    return null;
};

const encodeJson = (value: object): string => toBase64Url(new TextEncoder().encode(JSON.stringify(value)));

/** The payload for the relay. */
export const encodeKeyMessage = (message: KeyMessage): string => {
    if (message.type === 'hello') {
        return encodeJson({
            v: E2EE_VERSION,
            t: 'hello',
            d: toBase64Url(message.device),
            e: toBase64Url(message.ephemeral),
            s: toBase64Url(message.signature),
            ...(message.certificate ? { c: message.certificate } : {}),
        });
    }
    return encodeJson({
        v: E2EE_VERSION,
        t: 'key',
        k: message.keyId,
        i: toBase64Url(message.iv),
        w: toBase64Url(message.wrapped),
    });
};

/** What a hello's signature covers: the sender, the one receiver, and the sender's ECDH key for this call. */
export const helloSignedBytes = (from: string, to: string, ephemeral: Uint8Array): Uint8Array<ArrayBuffer> =>
    new TextEncoder().encode(`melior-e2ee/v${E2EE_VERSION}/hello|${from}|${to}|${toBase64Url(ephemeral)}`);

/** The additional data of a wrapped key: who sent which key id to whom. */
export const wrapAdditionalData = (from: string, to: string, keyId: number): Uint8Array<ArrayBuffer> =>
    new TextEncoder().encode(`melior-e2ee/v${E2EE_VERSION}/key|${from}|${to}|${keyId}`);

/** The HKDF salt of a pair's wrapping key: both session ids, sorted, so both sides derive the same key. */
export const pairSalt = (a: string, b: string): Uint8Array<ArrayBuffer> =>
    new TextEncoder().encode([a, b].sort().join('|'));

export const WRAP_KEY_INFO = new TextEncoder().encode(`melior-e2ee/v${E2EE_VERSION}/wrap`);
