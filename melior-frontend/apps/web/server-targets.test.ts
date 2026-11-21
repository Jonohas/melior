import { describe, expect, test } from 'vitest';
import { hashString, parseBackendUrls, pickBackend } from './server-targets';

describe('parseBackendUrls', () => {
    test('defaults to the local backend in http form', () => {
        expect(parseBackendUrls(undefined, undefined)).toEqual(['http://127.0.0.1:5000']);
    });

    test('a single MELIOR_SERVER_URL is converted to http(s) and loses trailing slashes', () => {
        expect(parseBackendUrls(undefined, 'ws://a:1/')).toEqual(['http://a:1']);
        expect(parseBackendUrls(undefined, 'wss://a')).toEqual(['https://a']);
        expect(parseBackendUrls('', 'http://b:2')).toEqual(['http://b:2']);
    });

    test('MELIOR_SERVER_URLS wins and keeps its order', () => {
        expect(parseBackendUrls('ws://a:1, http://b:2 ,,', 'ws://ignored:9')).toEqual(['http://a:1', 'http://b:2']);
    });
});

describe('pickBackend', () => {
    const targets = ['http://a', 'http://b', 'http://c'];

    test('is stable for a room and spreads rooms over the targets', () => {
        const picks = Array.from({ length: 60 }, (_, i) => pickBackend(targets, `room-${i}`));
        expect(picks).toEqual(Array.from({ length: 60 }, (_, i) => pickBackend(targets, `room-${i}`)));
        expect(new Set(picks)).toEqual(new Set(targets));
    });

    test('always returns the only target', () => {
        expect(pickBackend(['http://a'], 'x')).toBe('http://a');
        expect(pickBackend(['http://a'], null)).toBe('http://a');
    });

    test('hashString is deterministic', () => {
        expect(hashString('general')).toBe(hashString('general'));
        expect(hashString('a')).not.toBe(hashString('b'));
    });
});
