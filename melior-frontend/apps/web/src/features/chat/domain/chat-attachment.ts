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
 * Maps a saved message's `attachment` as the server sends it. Null for an unknown kind or a shape that does not
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

/** Whether a value is a saved attachment in its wire form. */
export const isChatAttachmentWire = (raw: unknown): raw is ChatAttachmentWire => parseChatAttachment(raw) !== null;

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
