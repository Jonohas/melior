/** RFC 2198 redundant audio: each packet also carries the previous frame, so one lost packet costs no sound. */
const RED_MIME_TYPE = 'audio/red';

const isRed = (codec: { mimeType: string }): boolean => codec.mimeType.toLowerCase() === RED_MIME_TYPE;

/**
 * The audio codecs with RED moved to the front, the rest in their order. A browser sends the first codec the
 * answer lists, so RED must lead the offer to be used at all. Undefined when RED is not among them: then the
 * browser's own order stays.
 */
export const redFirst = <T extends { mimeType: string }>(codecs: readonly T[]): T[] | undefined => {
    if (!codecs.some(isRed)) return undefined;
    return [...codecs.filter(isRed), ...codecs.filter((codec) => !isRed(codec))];
};
