/** The image types the server accepts for an attachment. It checks the bytes too; this only spares a doomed upload. */
export const IMAGE_TYPES: readonly string[] = ['image/png', 'image/jpeg', 'image/gif', 'image/webp'];

/** The largest image the server accepts: 10 MiB. */
export const MAX_IMAGE_BYTES = 10 * 1024 * 1024;

export type ImageFileCheck = { ok: true } | { ok: false; reason: 'empty' | 'unsupported_type' | 'too_large' };

/** Checks a chosen or pasted file before it is uploaded: a supported image type, not empty, at most 10 MiB. */
export const checkImageFile = (file: { type: string; size: number }): ImageFileCheck => {
    if (!IMAGE_TYPES.includes(file.type)) return { ok: false, reason: 'unsupported_type' };
    if (file.size <= 0) return { ok: false, reason: 'empty' };
    if (file.size > MAX_IMAGE_BYTES) return { ok: false, reason: 'too_large' };
    return { ok: true };
};

/** What to tell the person who picked a file that cannot be sent. */
export const describeImageRefusal = (reason: Extract<ImageFileCheck, { ok: false }>['reason']): string => {
    switch (reason) {
        case 'empty':
            return 'That file is empty.';
        case 'unsupported_type':
            return 'Only PNG, JPEG, GIF and WebP images can be sent.';
        case 'too_large':
            return 'An image is at most 10 MiB.';
    }
};
