import { parseUploadedImage, type UploadedImage } from '@/features/chat/domain';
import { UnauthorizedError } from '@/lib/auth';

/** Where a saved image is served; same origin with the session cookie, so a plain `<img src>` loads it. */
export const attachmentUrl = (id: string): string => `/api/attachments/${encodeURIComponent(id)}`;

const uploadUrl = (channelId: string): string => `/api/channels/${encodeURIComponent(channelId)}/attachments`;

const reasonOf = async (response: Response): Promise<string | null> => {
    const text = (await response.text().catch(() => '')).trim();
    return text !== '' && text.length < 200 ? text : null;
};

const uploadRefusal = async (response: Response): Promise<string> => {
    switch (response.status) {
        case 413:
            return 'The image is too large. An image is at most 10 MiB.';
        case 404:
            return 'This channel does not exist.';
        case 503:
            return 'Image uploads are not available on this server.';
        default:
            return (await reasonOf(response)) ?? `The image was not uploaded (HTTP ${response.status}).`;
    }
};

/**
 * Uploads an image for a message in a channel; the server checks the bytes and keeps it. Rejects with a reason
 * fit to show when it is refused.
 */
export const uploadImage = async (channelId: string, file: Blob): Promise<UploadedImage> => {
    let response: Response;
    try {
        response = await fetch(uploadUrl(channelId), {
            method: 'POST',
            headers: { 'Content-Type': file.type },
            body: file,
        });
    } catch {
        throw new Error('The image was not uploaded: the server could not be reached.');
    }
    if (response.status === 401) throw new UnauthorizedError();
    if (!response.ok) throw new Error(await uploadRefusal(response));
    const uploaded = parseUploadedImage(await response.json().catch(() => null));
    if (!uploaded) throw new Error('Unexpected upload response');
    return uploaded;
};
