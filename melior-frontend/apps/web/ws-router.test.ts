import { describe, expect, test } from 'vitest';
import { backendEndpoint, serializeUpgradeRequest, upgradeRoutingKey } from './ws-router';

describe('backendEndpoint', () => {
    test('reads host and port, with the scheme defaults', () => {
        expect(backendEndpoint('http://127.0.0.1:5000')).toEqual({ host: '127.0.0.1', port: 5000, secure: false });
        expect(backendEndpoint('ws://a')).toEqual({ host: 'a', port: 80, secure: false });
        expect(backendEndpoint('https://b')).toEqual({ host: 'b', port: 443, secure: true });
        expect(backendEndpoint('wss://b:8443')).toEqual({ host: 'b', port: 8443, secure: true });
    });
});

describe('upgradeRoutingKey', () => {
    test('routes the room socket by its channel id', () => {
        expect(upgradeRoutingKey('/ws?channel_id=a&x=1')).toBe('a');
        expect(upgradeRoutingKey('/ws')).toBe('');
    });

    test('routes the community feed by its community id', () => {
        expect(upgradeRoutingKey('/ws/communities/c-1')).toBe('c-1');
        expect(upgradeRoutingKey('/ws/communities/a%20b')).toBe('a b');
    });

    test('leaves Vite internals and lookalikes alone', () => {
        expect(upgradeRoutingKey('/')).toBeNull();
        expect(upgradeRoutingKey('/?token=abc')).toBeNull();
        expect(upgradeRoutingKey('/wsx')).toBeNull();
        expect(upgradeRoutingKey('/ws/other')).toBeNull();
        expect(upgradeRoutingKey('/ws/communities/')).toBeNull();
        expect(upgradeRoutingKey('/ws/communities/a/b')).toBeNull();
        expect(upgradeRoutingKey(undefined)).toBeNull();
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
