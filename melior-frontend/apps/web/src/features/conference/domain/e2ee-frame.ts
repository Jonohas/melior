/**
 * The layout of an end-to-end encrypted media frame. Pure byte arithmetic; the cipher itself runs in `api/`.
 *
 *     | clear prefix | ciphertext | GCM tag (16) | counter (8) | key id (4) | magic (1) |
 *
 * The clear prefix keeps the codec header readable: the receiver's depacketizer sees a valid frame start, and
 * the SFU can tell a VP8 keyframe without the key. The trailer names the key and the nonce.
 */

/** Marks a frame as encrypted by this scheme, version 1. */
export const FRAME_MAGIC = 0xe1;
export const TAG_BYTES = 16;
export const TRAILER_BYTES = 8 + 4 + 1;
export const NONCE_BYTES = 12;

/** A VP8 keyframe header: the 3-byte frame tag, the start code and the dimensions (RFC 6386, 9.1). */
const VP8_KEYFRAME_CLEAR = 10;
/** A VP8 delta frame header: the 3-byte frame tag. */
const VP8_DELTA_CLEAR = 3;
/** The Opus TOC byte (RFC 6716, 3.1). */
const OPUS_CLEAR = 1;

export type FrameMedia = 'audio' | 'video';

/** `type` of an `RTCEncodedVideoFrame`; audio frames have none. */
export type FrameType = 'key' | 'delta' | 'empty' | undefined;

/** How many leading bytes of a frame stay in the clear. Never more than the frame has. */
export const clearPrefixLength = (media: FrameMedia, type: FrameType, length: number): number => {
    const wanted = media === 'audio' ? OPUS_CLEAR : type === 'key' ? VP8_KEYFRAME_CLEAR : VP8_DELTA_CLEAR;
    return Math.min(wanted, length);
};

/**
 * The AES-GCM nonce: the key id and the frame counter. A key id is random per key and the counter never repeats
 * under one key, so no nonce repeats under one key.
 */
export const frameNonce = (keyId: number, counter: number): Uint8Array<ArrayBuffer> => {
    const nonce = new Uint8Array(NONCE_BYTES);
    const view = new DataView(nonce.buffer);
    view.setUint32(0, keyId);
    view.setBigUint64(4, BigInt(counter));
    return nonce;
};

export const writeTrailer = (keyId: number, counter: number): Uint8Array<ArrayBuffer> => {
    const trailer = new Uint8Array(TRAILER_BYTES);
    const view = new DataView(trailer.buffer);
    view.setBigUint64(0, BigInt(counter));
    view.setUint32(8, keyId);
    trailer[12] = FRAME_MAGIC;
    return trailer;
};

export type Trailer = { keyId: number; counter: number };

/**
 * Reads the trailer of an encrypted frame of `length` bytes with `clear` bytes in the clear. Null when the frame
 * is too short to hold a tag and a trailer, or carries no magic byte: it was not encrypted by this scheme.
 */
export const readTrailer = (frame: Uint8Array, clear: number): Trailer | null => {
    if (frame.length < clear + TAG_BYTES + TRAILER_BYTES) return null;
    const start = frame.length - TRAILER_BYTES;
    if (frame[frame.length - 1] !== FRAME_MAGIC) return null;
    const view = new DataView(frame.buffer, frame.byteOffset + start, TRAILER_BYTES);
    return { counter: Number(view.getBigUint64(0)), keyId: view.getUint32(8) };
};
