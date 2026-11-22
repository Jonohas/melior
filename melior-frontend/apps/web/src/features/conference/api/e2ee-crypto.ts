import {
    helloSignedBytes,
    pairSalt,
    toBase64Url,
    WRAP_KEY_INFO,
    type WrappedKey,
    wrapAdditionalData,
} from '@/features/conference/domain';

/**
 * The WebCrypto operations of end-to-end encryption. Every private key is created non-extractable: JavaScript
 * can use it, but not read its bytes. Only a media key exists as raw bytes, for the moment it takes to wrap it
 * for each peer.
 */

const ECDSA: EcKeyImportParams = { name: 'ECDSA', namedCurve: 'P-256' };
const ECDH: EcKeyImportParams = { name: 'ECDH', namedCurve: 'P-256' };
const SIGNING: EcdsaParams = { name: 'ECDSA', hash: 'SHA-256' };
const MEDIA_KEY_BYTES = 16;
const WRAP_IV_BYTES = 12;

export type DeviceKey = { privateKey: CryptoKey; publicRaw: Uint8Array<ArrayBuffer> };
export type CallKey = { privateKey: CryptoKey; publicRaw: Uint8Array<ArrayBuffer> };

/** A media key: the AES-GCM key frames are encrypted with, and its random id. */
export type MediaKey = { keyId: number; key: CryptoKey; raw: Uint8Array<ArrayBuffer> };

const exportRaw = async (key: CryptoKey): Promise<Uint8Array<ArrayBuffer>> =>
    new Uint8Array(await crypto.subtle.exportKey('raw', key));

/** This browser's long-lived signing key. Created once and kept by `device-store`. */
export const createDeviceKey = async (): Promise<DeviceKey> => {
    const pair = await crypto.subtle.generateKey(ECDSA, false, ['sign', 'verify']);
    return { privateKey: pair.privateKey, publicRaw: await exportRaw(pair.publicKey) };
};

/** The ECDH key of one call. It lives in memory only and dies with the call. */
export const createCallKey = async (): Promise<CallKey> => {
    const pair = await crypto.subtle.generateKey(ECDH, false, ['deriveBits']);
    return { privateKey: pair.privateKey, publicRaw: await exportRaw(pair.publicKey) };
};

/** Signs this call's ECDH key for one receiver, binding it to both session ids. */
export const signHello = async (device: DeviceKey, from: string, to: string, ephemeral: Uint8Array) =>
    new Uint8Array(await crypto.subtle.sign(SIGNING, device.privateKey, helloSignedBytes(from, to, ephemeral)));

/** Whether `signature` is the device key's signature over the hello `from` sent to `to`. */
export const verifyHello = async (
    device: Uint8Array<ArrayBuffer>,
    from: string,
    to: string,
    ephemeral: Uint8Array<ArrayBuffer>,
    signature: Uint8Array<ArrayBuffer>,
): Promise<boolean> => {
    try {
        const publicKey = await crypto.subtle.importKey('raw', device, ECDSA, false, ['verify']);
        return await crypto.subtle.verify(SIGNING, publicKey, signature, helloSignedBytes(from, to, ephemeral));
    } catch {
        return false;
    }
};

/** The AES-GCM key two participants wrap media keys with: ECDH of their call keys, through HKDF-SHA-256. */
export const derivePairKey = async (
    own: CallKey,
    peerPublic: Uint8Array<ArrayBuffer>,
    selfId: string,
    peerId: string,
): Promise<CryptoKey> => {
    const peerKey = await crypto.subtle.importKey('raw', peerPublic, ECDH, false, []);
    const bits = await crypto.subtle.deriveBits({ name: 'ECDH', public: peerKey }, own.privateKey, 256);
    const secret = await crypto.subtle.importKey('raw', bits, 'HKDF', false, ['deriveKey']);
    return crypto.subtle.deriveKey(
        { name: 'HKDF', hash: 'SHA-256', salt: pairSalt(selfId, peerId), info: WRAP_KEY_INFO },
        secret,
        { name: 'AES-GCM', length: 256 },
        false,
        ['encrypt', 'decrypt'],
    );
};

const importMediaKey = (raw: Uint8Array<ArrayBuffer>): Promise<CryptoKey> =>
    crypto.subtle.importKey('raw', raw, 'AES-GCM', false, ['encrypt', 'decrypt']);

/** A fresh random media key with a random 32-bit id. */
export const createMediaKey = async (): Promise<MediaKey> => {
    const raw = crypto.getRandomValues(new Uint8Array(MEDIA_KEY_BYTES));
    const [keyId = 0] = crypto.getRandomValues(new Uint32Array(1));
    return { keyId, key: await importMediaKey(raw), raw };
};

/** Encrypts a media key for one peer. */
export const wrapMediaKey = async (
    pairKey: CryptoKey,
    media: MediaKey,
    from: string,
    to: string,
): Promise<WrappedKey> => {
    const iv = crypto.getRandomValues(new Uint8Array(WRAP_IV_BYTES));
    const additionalData = wrapAdditionalData(from, to, media.keyId);
    const wrapped = await crypto.subtle.encrypt({ name: 'AES-GCM', iv, additionalData }, pairKey, media.raw);
    return { type: 'key', keyId: media.keyId, iv, wrapped: new Uint8Array(wrapped) };
};

/** Decrypts a media key a peer sent; null when it does not decrypt (wrong pair key, tampered, misaddressed). */
export const unwrapMediaKey = async (
    pairKey: CryptoKey,
    message: WrappedKey,
    from: string,
    to: string,
): Promise<CryptoKey | null> => {
    try {
        const additionalData = wrapAdditionalData(from, to, message.keyId);
        const raw = await crypto.subtle.decrypt(
            { name: 'AES-GCM', iv: message.iv, additionalData },
            pairKey,
            message.wrapped,
        );
        return await importMediaKey(new Uint8Array(raw));
    } catch {
        return null;
    }
};

export const sha256 = async (data: string | Uint8Array<ArrayBuffer>): Promise<Uint8Array<ArrayBuffer>> =>
    new Uint8Array(
        await crypto.subtle.digest('SHA-256', typeof data === 'string' ? new TextEncoder().encode(data) : data),
    );

/** `bytes` random bytes, base64url-encoded: a PKCE verifier, an OAuth `state`, a JWT `jti`. */
export const randomText = (bytes: number): string => toBase64Url(crypto.getRandomValues(new Uint8Array(bytes)));
