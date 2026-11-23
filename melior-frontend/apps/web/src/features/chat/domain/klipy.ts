import { type GifAttachment, parseGif } from './chat-attachment';

/** How many GIFs one KLIPY request asks for. */
export const GIF_PAGE_SIZE = 24;

/** One GIF in the picker: a small rendition for the grid and the one that is sent. */
export type GifResult = {
    id: string;
    title: string;
    preview: { url: string; width: number; height: number };
    gif: GifAttachment;
};

export type GifPage = {
    results: GifResult[];
    hasNext: boolean;
    page: number;
};

type Media = { url: string; width: number; height: number };

const isObject = (value: unknown): value is Record<string, unknown> =>
    typeof value === 'object' && value !== null && !Array.isArray(value);

const isPositiveInt = (value: unknown): value is number =>
    typeof value === 'number' && Number.isInteger(value) && value > 0;

/** `file[size][format]`, when it is a usable https rendition. */
const mediaAt = (file: Record<string, unknown>, size: string, format: string): Media | null => {
    const sized = file[size];
    if (!isObject(sized)) return null;
    const media = sized[format];
    if (!isObject(media)) return null;
    const { url, width, height } = media;
    if (typeof url !== 'string' || !url.startsWith('https://')) return null;
    if (!isPositiveInt(width) || !isPositiveInt(height)) return null;
    return { url, width, height };
};

const firstMedia = (file: Record<string, unknown>, choices: readonly (readonly [string, string])[]): Media | null => {
    for (const [size, format] of choices) {
        const media = mediaAt(file, size, format);
        if (media) return media;
    }
    return null;
};

const PREVIEW_CHOICES = [
    ['xs', 'webp'],
    ['xs', 'gif'],
    ['sm', 'webp'],
    ['sm', 'gif'],
] as const;

const SENT_CHOICES = [
    ['md', 'gif'],
    ['sm', 'gif'],
] as const;

/** One search or trending entry; null when it has no rendition to show or none the server would accept. */
const parseGifResult = (raw: unknown): GifResult | null => {
    if (!isObject(raw) || !isObject(raw.file)) return null;
    const id = typeof raw.id === 'number' || typeof raw.id === 'string' ? String(raw.id) : null;
    if (id === null) return null;
    const preview = firstMedia(raw.file, PREVIEW_CHOICES);
    const sent = firstMedia(raw.file, SENT_CHOICES);
    const gif = sent ? parseGif(sent) : null;
    if (!preview || !gif) return null;
    return { id, title: typeof raw.title === 'string' ? raw.title : '', preview, gif };
};

/**
 * Maps a KLIPY search or trending response, which is untrusted. Null when the envelope is not the documented
 * shape; entries keep their order, and only those without a usable file are left out.
 */
export const parseGifPage = (raw: unknown): GifPage | null => {
    if (!isObject(raw) || raw.result !== true || !isObject(raw.data)) return null;
    const { data, has_next, current_page } = raw.data;
    if (!Array.isArray(data) || typeof has_next !== 'boolean') return null;
    const results = data.map(parseGifResult).filter((result): result is GifResult => result !== null);
    const page = typeof current_page === 'number' && Number.isSafeInteger(current_page) ? current_page : 1;
    return { results, hasNext: has_next, page };
};

/**
 * Appends a further page to what is shown, leaving out GIFs that are there already: a trending list can shift
 * between two requests, and the grid shows each GIF once.
 */
export const appendGifResults = (current: readonly GifResult[], next: readonly GifResult[]): GifResult[] => {
    const ids = new Set(current.map((result) => result.id));
    const added: GifResult[] = [];
    for (const result of next) {
        if (ids.has(result.id)) continue;
        ids.add(result.id);
        added.push(result);
    }
    return [...current, ...added];
};

/** The search a query asks for: trending when it is blank, else the trimmed query. */
export const gifQueryOf = (input: string): { kind: 'trending' } | { kind: 'search'; query: string } => {
    const query = input.trim();
    return query === '' ? { kind: 'trending' } : { kind: 'search', query };
};

/** Parses `GET /api/gifs/config`: the KLIPY app key, or null when GIFs are off on this server. */
export const parseGifConfig = (raw: unknown): { klipyAppKey: string | null } | null => {
    if (!isObject(raw)) return null;
    const key = raw.klipy_app_key;
    if (key === null) return { klipyAppKey: null };
    if (typeof key !== 'string') return null;
    return { klipyAppKey: key.trim() === '' ? null : key };
};
