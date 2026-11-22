/**
 * RED (RFC 2198) audio payloads, as Chromium hands them to an encoded transform when a section negotiated
 * `audio/red`: block headers, then the redundant blocks (earlier frames), then the primary block (this frame).
 *
 * End-to-end encryption must keep the headers readable, because the SFU unwraps RED for a participant without it
 * and renumbers it for one with it. So each block is encrypted on its own, and its length in the header is
 * rewritten to match.
 */

/** One block: its payload type and timestamp offset (both from the header), and its bytes. */
export type RedBlock = { payloadType: number; timestampOffset: number; data: Uint8Array };

export type RedPayload = { redundant: RedBlock[]; primary: RedBlock };

/** The largest length a redundant block's header can state (10 bits). */
const MAX_BLOCK_LENGTH = 0x3ff;
const MAX_TIMESTAMP_OFFSET = 0x3fff;

type Header = { payloadType: number; timestampOffset: number; length: number };

/** The block headers and where the data starts; null when the headers run past the payload. */
const readHeaders = (payload: Uint8Array): { headers: Header[]; primaryType: number; offset: number } | null => {
    const headers: Header[] = [];
    let offset = 0;
    for (;;) {
        const first = payload[offset];
        if (first === undefined) return null;
        if ((first & 0x80) === 0) return { headers, primaryType: first & 0x7f, offset: offset + 1 };
        const rest = payload.subarray(offset + 1, offset + 4);
        if (rest.length < 3) return null;
        const [b1 = 0, b2 = 0, b3 = 0] = rest;
        headers.push({
            payloadType: first & 0x7f,
            timestampOffset: (b1 << 6) | (b2 >> 2),
            length: ((b2 & 0x03) << 8) | b3,
        });
        offset += 4;
    }
};

/** Splits a RED payload into its blocks; null when it is malformed (the lengths do not fit the payload). */
export const parseRed = (payload: Uint8Array): RedPayload | null => {
    const read = readHeaders(payload);
    if (!read) return null;
    let offset = read.offset;
    const redundant: RedBlock[] = [];
    for (const header of read.headers) {
        if (offset + header.length > payload.length) return null;
        redundant.push({
            payloadType: header.payloadType,
            timestampOffset: header.timestampOffset,
            data: payload.subarray(offset, offset + header.length),
        });
        offset += header.length;
    }
    return {
        redundant,
        primary: { payloadType: read.primaryType, timestampOffset: 0, data: payload.subarray(offset) },
    };
};

/**
 * Builds a RED payload from blocks. A redundant block too long for its header, or with an offset out of range,
 * is left out: the frame still carries its primary block.
 */
export const buildRed = ({ redundant, primary }: RedPayload): Uint8Array<ArrayBuffer> => {
    const kept = redundant.filter(
        (block) => block.data.length <= MAX_BLOCK_LENGTH && block.timestampOffset <= MAX_TIMESTAMP_OFFSET,
    );
    const size = kept.length * 4 + 1 + kept.reduce((sum, block) => sum + block.data.length, 0) + primary.data.length;
    const out = new Uint8Array(size);
    let offset = 0;
    for (const block of kept) {
        out[offset] = 0x80 | (block.payloadType & 0x7f);
        out[offset + 1] = (block.timestampOffset >> 6) & 0xff;
        out[offset + 2] = ((block.timestampOffset & 0x3f) << 2) | ((block.data.length >> 8) & 0x03);
        out[offset + 3] = block.data.length & 0xff;
        offset += 4;
    }
    out[offset] = primary.payloadType & 0x7f;
    offset += 1;
    for (const block of [...kept, primary]) {
        out.set(block.data, offset);
        offset += block.data.length;
    }
    return out;
};
