import { describe, expect, test } from 'vitest';
import { backendEndpoint, isSignallingUpgrade, serializeUpgradeRequest } from './ws-router';

describe('backendEndpoint', () => {
    test('reads host and port, with the scheme defaults', () => {
        expect(backendEndpoint('http://127.0.0.1:5000')).toEqual({ host: '127.0.0.1', port: 5000, secure: false });
        expect(backendEndpoint('ws://a')).toEqual({ host: 'a', port: 80, secure: false });
        expect(backendEndpoint('https://b')).toEqual({ host: 'b', port: 443, secure: true });
        expect(backendEndpoint('wss://b:8443')).toEqual({ host: 'b', port: 8443, secure: true });
    });
});

describe('isSignallingUpgrade', () => {
    test('matches /ws only, not Vite internals or lookalikes', () => {
        expect(isSignallingUpgrade('/ws?channel_id=a')).toBe(true);
        expect(isSignallingUpgrade('/ws')).toBe(true);
        expect(isSignallingUpgrade('/')).toBe(false);
        expect(isSignallingUpgrade('/?token=abc')).toBe(false);
        expect(isSignallingUpgrade('/wsx')).toBe(false);
        expect(isSignallingUpgrade('/ws/other')).toBe(false);
        expect(isSignallingUpgrade(undefined)).toBe(false);
    });
});

describe('serializeUpgradeRequest', () => {
    test('keeps the request line and the raw headers in order and case', () => {
        const raw = [
            'Host',
            'localhost:3000',
            'Origin',
            'http://localhost:3000',
            'Cookie',
            'a=b; c=d',
            'Upgrade',
            'websocket',
        ];
        expect(serializeUpgradeRequest('GET', '/ws?channel_id=x', '1.1', raw)).toBe(
            'GET /ws?channel_id=x HTTP/1.1\r\nHost: localhost:3000\r\nOrigin: http://localhost:3000\r\nCookie: a=b; c=d\r\nUpgrade: websocket\r\n\r\n',
        );
    });
});
