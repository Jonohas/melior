/** A GIF as it is sent and saved: the KLIPY media URL and its size in pixels. */
export type GifAttachment = { kind: 'gif'; url: string; width: number; height: number };

/** What a message carries besides its text, as the server saved it. */
export type ChatAttachment = { kind: 'image'; id: string; contentType: string } | GifAttachment;

/**
 * What a message to be sent carries: an uploaded image by its id, or a GIF. The field names are the wire names, so
 * it goes into a `chat_send` payload or a POST body as it is.
 */
export type OutgoingAttachment = { kind: 'image'; id: string } | GifAttachment;

/** A saved attachment as the server sends it, for the messages that carry the wire form on. */
export type ChatAttachmentWire = { kind: 'image'; id: string; content_type: string } | GifAttachment;

/** The hosts KLIPY serves its media from; the server accepts GIFs from these only. */
export const GIF_HOSTS: readonly string[] = ['static.klipy.com', 'static1.klipy.com', 'static2.klipy.com'];

/** The longest GIF URL the server accepts. */
export const MAX_GIF_URL_CHARS = 1024;

/** The largest width or height of a GIF the server accepts, in pixels. */
export const MAX_GIF_SIDE = 4096;

const isObject = (value: unknown): value is Record<string, unknown> =>
    typeof value === 'object' && value !== null && !Array.isArray(value);

const isSide = (value: unknown): value is number =>
    typeof value === 'number' && Number.isInteger(value) && value >= 1 && value <= MAX_GIF_SIDE;

/** Whether a URL is one the server takes for a GIF: https, on a KLIPY media host, and not too long. */
export const isAllowedGifUrl = (url: string): boolean => {
    if (url.length > MAX_GIF_URL_CHARS) return false;
    let parsed: URL;
    try {
        parsed = new URL(url);
    } catch {
        return false;
    }
    // `host` includes a port, so a URL with an explicit port is refused as well.
    return parsed.protocol === 'https:' && GIF_HOSTS.includes(parsed.host) && parsed.username === '';
};

/** Checks a GIF the way the server does: an allowed URL and whole-pixel sides of 1 to `MAX_GIF_SIDE`. */
export const parseGif = (raw: unknown): GifAttachment | null => {
    if (!isObject(raw)) return null;
    const { url, width, height } = raw;
    if (typeof url !== 'string' || !isAllowedGifUrl(url) || !isSide(width) || !isSide(height)) return null;
    return { kind: 'gif', url, width, height };
};

/**
 * Maps one saved attachment as the server sends it. Null for an unknown kind or a shape that does not
 * fit, which makes the whole message invalid: a GIF whose URL the server would not have accepted is not shown.
 */
export const parseChatAttachment = (raw: unknown): ChatAttachment | null => {
    if (!isObject(raw)) return null;
    switch (raw.kind) {
        case 'image': {
            const { id, content_type } = raw;
            if (typeof id !== 'string' || id === '' || typeof content_type !== 'string' || content_type === '')
                return null;
            return { kind: 'image', id, contentType: content_type };
        }
        case 'gif':
            return parseGif(raw);
        default:
            return null;
    }
};

/**
 * Maps a saved message's `attachments` list, in order. Absent means none; anything that is not a list, or a list
 * with one item that does not fit, is null, which makes the whole message invalid.
 */
export const parseChatAttachments = (raw: unknown): ChatAttachment[] | null => {
    if (raw === undefined) return [];
    if (!Array.isArray(raw)) return null;
    const attachments = raw.map(parseChatAttachment);
    return attachments.every((attachment): attachment is ChatAttachment => attachment !== null) ? attachments : null;
};

/** Whether a value is a saved message's attachment list in its wire form. */
export const isChatAttachmentsWire = (raw: unknown): raw is ChatAttachmentWire[] =>
    Array.isArray(raw) && parseChatAttachments(raw) !== null;

/** The most images one message may carry. */
export const MAX_IMAGES_PER_MESSAGE = 10;

export type AttachmentsCheck =
    | { ok: true }
    | { ok: false; reason: 'too_many_images' | 'gif_not_alone' | 'duplicate_image' };

/**
 * Checks what a message to be sent carries, the way the server does: none, 1 to `MAX_IMAGES_PER_MESSAGE` images
 * with no id twice, or exactly one GIF on its own.
 */
export const checkAttachments = (attachments: readonly OutgoingAttachment[]): AttachmentsCheck => {
    const gifs = attachments.filter((attachment) => attachment.kind === 'gif').length;
    if (gifs > 0 && attachments.length > 1) return { ok: false, reason: 'gif_not_alone' };
    const ids = attachments.flatMap((attachment) => (attachment.kind === 'image' ? [attachment.id] : []));
    if (ids.length > MAX_IMAGES_PER_MESSAGE) return { ok: false, reason: 'too_many_images' };
    if (new Set(ids).size !== ids.length) return { ok: false, reason: 'duplicate_image' };
    return { ok: true };
};

/**
 * Admits picked images to a draft that holds `present` already: the first ones up to `MAX_IMAGES_PER_MESSAGE`,
 * in the order they were picked. `leftOut` counts the rest, to tell the user.
 */
export const admitImages = <T>(present: number, picked: readonly T[]): { admitted: T[]; leftOut: number } => {
    const room = Math.max(0, MAX_IMAGES_PER_MESSAGE - present);
    const admitted = picked.slice(0, room);
    return { admitted, leftOut: picked.length - admitted.length };
};

/** What to tell the user when picked images did not fit in the message. */
export const describeLeftOut = (leftOut: number): string =>
    `A message carries at most ${MAX_IMAGES_PER_MESSAGE} images; ${leftOut === 1 ? '1 image was' : `${leftOut} images were`} left out.`;

/** An image the server has stored, ready to be sent by its id. */
export type UploadedImage = { id: string; contentType: string; size: number };

/** Parses the answer to `POST /api/channels/{id}/attachments`; null when it is not the documented shape. */
export const parseUploadedImage = (raw: unknown): UploadedImage | null => {
    if (!isObject(raw)) return null;
    const { id, content_type, size } = raw;
    if (typeof id !== 'string' || id === '' || typeof content_type !== 'string') return null;
    if (typeof size !== 'number' || !Number.isSafeInteger(size) || size < 0) return null;
    return { id, contentType: content_type, size };
};

/** The largest side an attachment is shown with in the message list, in CSS pixels. */
export const ATTACHMENT_BOX_PX = 320;

/** Scales a size down (never up) to fit a square box, keeping its aspect ratio and whole pixels of at least 1. */
export const fitWithin = (
    width: number,
    height: number,
    box: number = ATTACHMENT_BOX_PX,
): { width: number; height: number } => {
    const scale = Math.min(1, box / width, box / height);
    return { width: Math.max(1, Math.round(width * scale)), height: Math.max(1, Math.round(height * scale)) };
};
