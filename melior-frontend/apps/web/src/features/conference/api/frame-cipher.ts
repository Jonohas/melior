import {
    clearPrefixLength,
    type FrameMedia,
    type FrameType,
    frameNonce,
    readTrailer,
    TRAILER_BYTES,
    writeTrailer,
} from '@/features/conference/domain';

/** The key a sender encrypts with, and how many frames it has encrypted under it. */
export type SendingKey = { keyId: number; key: CryptoKey; counter: number };

/** Finds the candidate keys for a key id: every peer's keys, by id. Ids are random, so a clash is rare. */
export type KeyLookup = (keyId: number) => readonly CryptoKey[];

/**
 * Encrypts one encoded frame. Mutates `sending.counter`, which must be the only counter of that key: one
 * worker serves every sender of a call. The clear prefix and the trailer are authenticated as additional data.
 */
export const encryptFrame = async (
    data: ArrayBuffer,
    media: FrameMedia,
    type: FrameType,
    sending: SendingKey,
): Promise<ArrayBuffer> => {
    const frame = new Uint8Array(data);
    const clear = clearPrefixLength(media, type, frame.length);
    const counter = sending.counter;
    sending.counter += 1;
    const trailer = writeTrailer(sending.keyId, counter);
    const additionalData = new Uint8Array(clear + TRAILER_BYTES);
    additionalData.set(frame.subarray(0, clear));
    additionalData.set(trailer, clear);
    const sealed = new Uint8Array(
        await crypto.subtle.encrypt(
            { name: 'AES-GCM', iv: frameNonce(sending.keyId, counter), additionalData },
            sending.key,
            frame.subarray(clear),
        ),
    );
    const out = new Uint8Array(clear + sealed.length + TRAILER_BYTES);
    out.set(frame.subarray(0, clear));
    out.set(sealed, clear);
    out.set(trailer, clear + sealed.length);
    return out.buffer;
};

/**
 * Decrypts one encoded frame with whichever key of `lookup` opens it. Null when it was not encrypted by this
 * scheme, its key is unknown, or it was tampered with: the caller drops it, never hands it to the decoder.
 */
export const decryptFrame = async (
    data: ArrayBuffer,
    media: FrameMedia,
    type: FrameType,
    lookup: KeyLookup,
): Promise<ArrayBuffer | null> => {
    const frame = new Uint8Array(data);
    const clear = clearPrefixLength(media, type, frame.length);
    const trailer = readTrailer(frame, clear);
    if (!trailer) return null;
    const sealedEnd = frame.length - TRAILER_BYTES;
    const additionalData = new Uint8Array(clear + TRAILER_BYTES);
    additionalData.set(frame.subarray(0, clear));
    additionalData.set(frame.subarray(sealedEnd), clear);
    const iv = frameNonce(trailer.keyId, trailer.counter);
    for (const key of lookup(trailer.keyId)) {
        try {
            const opened = new Uint8Array(
                await crypto.subtle.decrypt(
                    { name: 'AES-GCM', iv, additionalData },
                    key,
                    frame.subarray(clear, sealedEnd),
                ),
            );
            const out = new Uint8Array(clear + opened.length);
            out.set(frame.subarray(0, clear));
            out.set(opened, clear);
            return out.buffer;
        } catch {
            // Not this key; a clash of random ids is possible, so the next candidate is tried.
        }
    }
    return null;
};
