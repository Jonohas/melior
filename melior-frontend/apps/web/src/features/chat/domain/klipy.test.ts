import { describe, expect, test } from 'vitest';
import { appendGifResults, type GifResult, gifQueryOf, parseGifConfig, parseGifPage } from './klipy';

const media = (url: string, width = 200, height = 100) => ({ url, width, height, size: 1000 });
const item = (id: number, file: unknown = full(id)) => ({ id, slug: `s${id}`, title: `t${id}`, type: 'gif', file });
const full = (id: number) => ({
    md: { gif: media(`https://static.klipy.com/${id}/md.gif`, 498, 280) },
    sm: { gif: media(`https://static.klipy.com/${id}/sm.gif`), webp: media(`https://static.klipy.com/${id}/sm.webp`) },
    xs: { webp: media(`https://static.klipy.com/${id}/xs.webp`, 100, 56) },
});
const envelope = (data: unknown[], hasNext = true, page = 1) => ({
    result: true,
    data: { data, has_next: hasNext, current_page: page, per_page: 24 },
});

describe('parseGifPage', () => {
    test('uses xs for the preview and md.gif for the GIF that is sent', () => {
        const page = parseGifPage(envelope([item(1)]));
        expect(page).toEqual({
            results: [
                {
                    id: '1',
                    title: 't1',
                    preview: { url: 'https://static.klipy.com/1/xs.webp', width: 100, height: 56 },
                    gif: { kind: 'gif', url: 'https://static.klipy.com/1/md.gif', width: 498, height: 280 },
                },
            ],
            hasNext: true,
            page: 1,
        });
    });

    test('falls back to sm for the preview and to sm.gif for the sent GIF', () => {
        const file = { sm: { gif: media('https://static1.klipy.com/2/sm.gif', 220, 120) } };
        const [result] = parseGifPage(envelope([item(2, file)]))?.results ?? [];
        expect(result?.preview).toEqual({ url: 'https://static1.klipy.com/2/sm.gif', width: 220, height: 120 });
        expect(result?.gif).toEqual({
            kind: 'gif',
            url: 'https://static1.klipy.com/2/sm.gif',
            width: 220,
            height: 120,
        });
    });

    test('keeps the order and skips only entries without a usable file', () => {
        const noGif = { xs: { webp: media('https://static.klipy.com/3/xs.webp') } };
        const foreignHost = { md: { gif: media('https://cdn.example/4.gif') }, xs: full(4).xs };
        const http = { md: { gif: media('http://static.klipy.com/5.gif') }, xs: full(5).xs };
        const badSize = { md: { gif: media('https://static.klipy.com/6.gif', 0, 10) }, xs: full(6).xs };
        const page = parseGifPage(
            envelope([
                item(9),
                item(3, noGif),
                item(4, foreignHost),
                item(5, http),
                item(6, badSize),
                item(7, null),
                'junk',
                { ...item(8), id: undefined },
                item(1),
            ]),
        );
        expect(page?.results.map((result) => result.id)).toEqual(['9', '1']);
    });

    test('rejects an envelope that is not the documented shape', () => {
        for (const bad of [
            null,
            [],
            { result: false, data: { data: [], has_next: false } },
            { result: true },
            { result: true, data: { data: 'x', has_next: false } },
            { result: true, data: { data: [], has_next: 'no' } },
        ]) {
            expect(parseGifPage(bad)).toBeNull();
        }
    });

    test('an empty page is an empty result, not an error', () => {
        expect(parseGifPage(envelope([], false, 3))).toEqual({ results: [], hasNext: false, page: 3 });
    });
});

describe('appendGifResults', () => {
    test('adds the next page leaving out GIFs already shown', () => {
        const of = (id: string) => ({ id }) as GifResult;
        expect(appendGifResults([of('1'), of('2')], [of('2'), of('3'), of('3')]).map((r) => r.id)).toEqual([
            '1',
            '2',
            '3',
        ]);
    });
});

describe('gifQueryOf', () => {
    test('a blank input asks for trending, anything else for a trimmed search', () => {
        expect(gifQueryOf('  ')).toEqual({ kind: 'trending' });
        expect(gifQueryOf(' cats ')).toEqual({ kind: 'search', query: 'cats' });
    });
});

describe('parseGifConfig', () => {
    test('reads the app key, and null or a blank key means GIFs are off', () => {
        expect(parseGifConfig({ klipy_app_key: 'k' })).toEqual({ klipyAppKey: 'k' });
        expect(parseGifConfig({ klipy_app_key: null })).toEqual({ klipyAppKey: null });
        expect(parseGifConfig({ klipy_app_key: ' ' })).toEqual({ klipyAppKey: null });
        expect(parseGifConfig({})).toBeNull();
        expect(parseGifConfig({ klipy_app_key: 5 })).toBeNull();
    });
});
