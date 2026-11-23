import { useQuery } from '@tanstack/react-query';
import { useCallback, useEffect, useRef, useState } from 'react';
import { fetchGifConfig, fetchGifs } from '@/features/chat/api';
import { appendGifResults, type GifResult } from '@/features/chat/domain';
import { queryKeys } from '@/lib/query-keys';

/** The KLIPY app key, or null while it loads, when GIFs are off on this server, or when it cannot be read. */
export const useGifAppKey = (): string | null => {
    const query = useQuery({
        queryKey: queryKeys.gifs.config,
        queryFn: fetchGifConfig,
        staleTime: Number.POSITIVE_INFINITY,
        retry: 1,
    });
    return query.data ?? null;
};

/** How long typing has to pause before a search is sent. */
export const GIF_SEARCH_DEBOUNCE_MS = 300;

export type GifSearch = {
    results: GifResult[];
    status: 'loading' | 'ready' | 'error';
    loadingMore: boolean;
    hasNext: boolean;
    /** Why the last request failed; never contains the app key. */
    error: string | null;
    loadMore: () => void;
    retry: () => void;
};

type Listing = {
    query: string;
    results: GifResult[];
    page: number;
    hasNext: boolean;
    status: GifSearch['status'];
    loadingMore: boolean;
    error: string | null;
};

const failureOf = (error: unknown): string =>
    error instanceof Error && error.message.startsWith('KLIPY') ? error.message : 'Could not load GIFs.';

/** KLIPY's trending GIFs, or the results for `input` once typing pauses, a page at a time. */
export const useGifSearch = (appKey: string, input: string): GifSearch => {
    const [query, setQuery] = useState(input);
    const [attempt, setAttempt] = useState(0);
    const [listing, setListing] = useState<Listing>({
        query: input,
        results: [],
        page: 0,
        hasNext: false,
        status: 'loading',
        loadingMore: false,
        error: null,
    });
    const more = useRef<AbortController | null>(null);

    useEffect(() => {
        const timer = setTimeout(() => setQuery(input), GIF_SEARCH_DEBOUNCE_MS);
        return () => clearTimeout(timer);
    }, [input]);

    // biome-ignore lint/correctness/useExhaustiveDependencies: `attempt` only asks for the same search again
    useEffect(() => {
        const controller = new AbortController();
        more.current?.abort();
        setListing((current) => ({ ...current, query, status: 'loading', loadingMore: false, error: null }));
        fetchGifs(appKey, query, 1, controller.signal)
            .then((page) =>
                setListing({
                    query,
                    results: appendGifResults([], page.results),
                    page: 1,
                    hasNext: page.hasNext,
                    status: 'ready',
                    loadingMore: false,
                    error: null,
                }),
            )
            .catch((error: unknown) => {
                if (controller.signal.aborted) return;
                setListing((current) => ({ ...current, results: [], status: 'error', error: failureOf(error) }));
            });
        return () => controller.abort();
    }, [appKey, query, attempt]);

    useEffect(() => () => more.current?.abort(), []);

    const loadMore = useCallback(() => {
        if (listing.status !== 'ready' || listing.loadingMore || !listing.hasNext) return;
        const controller = new AbortController();
        more.current = controller;
        const next = listing.page + 1;
        setListing((current) => ({ ...current, loadingMore: true, error: null }));
        fetchGifs(appKey, listing.query, next, controller.signal)
            .then((page) =>
                setListing((current) =>
                    current.query === listing.query
                        ? {
                              ...current,
                              results: appendGifResults(current.results, page.results),
                              page: next,
                              hasNext: page.hasNext,
                              loadingMore: false,
                          }
                        : current,
                ),
            )
            .catch((error: unknown) => {
                if (controller.signal.aborted) return;
                setListing((current) => ({ ...current, loadingMore: false, error: failureOf(error) }));
            });
    }, [appKey, listing]);

    const retry = useCallback(() => setAttempt((count) => count + 1), []);

    return {
        results: listing.results,
        status: listing.status,
        loadingMore: listing.loadingMore,
        hasNext: listing.hasNext,
        error: listing.error,
        loadMore,
        retry,
    };
};
