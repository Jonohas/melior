import { GIF_PAGE_SIZE, type GifPage, gifQueryOf, parseGifConfig, parseGifPage } from '@/features/chat/domain';
import { UnauthorizedError } from '@/lib/auth';

const KLIPY_API = 'https://api.klipy.com/api/v1';

/** The KLIPY app key this server uses, or null when GIFs are off. */
export const fetchGifConfig = async (): Promise<string | null> => {
    const response = await fetch('/api/gifs/config');
    if (response.status === 401) throw new UnauthorizedError();
    if (!response.ok) throw new Error(`GIF config request failed with HTTP ${response.status}`);
    const config = parseGifConfig(await response.json());
    if (!config) throw new Error('Unexpected GIF config response');
    return config.klipyAppKey;
};

const gifsUrl = (appKey: string, input: string, page: number): string => {
    const search = gifQueryOf(input);
    const query = new URLSearchParams();
    if (search.kind === 'search') query.set('q', search.query);
    query.set('page', String(page));
    query.set('per_page', String(GIF_PAGE_SIZE));
    query.set('content_filter', 'medium');
    query.set('format_filter', 'gif,webp');
    return `${KLIPY_API}/${encodeURIComponent(appKey)}/gifs/${search.kind}?${query}`;
};

/**
 * One page of KLIPY's trending GIFs (blank input) or of a search. KLIPY wants these requests to come from the
 * user's browser, so this goes to KLIPY directly. The app key is part of the URL; it is kept out of every error.
 */
export const fetchGifs = async (
    appKey: string,
    input: string,
    page: number,
    signal?: AbortSignal,
): Promise<GifPage> => {
    let response: Response;
    try {
        response = await fetch(gifsUrl(appKey, input, page), { signal, credentials: 'omit', referrerPolicy: 'origin' });
    } catch (error) {
        if (signal?.aborted) throw error;
        throw new Error('KLIPY could not be reached.');
    }
    if (!response.ok) throw new Error(`KLIPY answered with HTTP ${response.status}.`);
    const parsed = parseGifPage(await response.json().catch(() => null));
    if (!parsed) throw new Error('KLIPY sent an unexpected response.');
    return parsed;
};
