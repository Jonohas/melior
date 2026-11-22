import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import {
    type DeviceTrust,
    encodeKeyMessage,
    HELLO_RETRY_MS,
    KEY_SWITCH_DELAY_MS,
    parseKeyMessage,
    type SessionId,
    toBase64Url,
} from '@/features/conference/domain';
import { createDeviceKey, type DeviceKey } from './e2ee-crypto';
import { type E2eeSession, startE2eeSession } from './e2ee-session';
import type { Voucher } from './e2ee-vouch';
import type { WorkerCommand } from './e2ee-worker';
import { decryptFrame, encryptFrame } from './frame-cipher';

type Party = {
    id: SessionId;
    session: E2eeSession;
    commands: WorkerCommand[];
    trust: Map<SessionId, DeviceTrust>;
    /** Messages this party sent, not yet delivered. */
    outbox: Array<{ to: SessionId; payload: string }>;
};

/**
 * A realm in miniature: it issues a certificate that binds a device key to a username, and checks one. A browser
 * without a certificate gets null, as when the realm cannot give one.
 */
const createRealm = (vouching = true) => {
    const issued = new Map<string, { device: string; username: string }>();
    const issue = (device: DeviceKey, username: string) => {
        const token = `token-${issued.size}`;
        issued.set(token, { device: toBase64Url(device.publicRaw), username });
        return token;
    };
    const voucher = (certificate: string | null): Voucher => ({
        vouching,
        ownCertificate: async () => certificate,
        renewIfNeeded: async () => false,
        verify: async (token, device, username) => {
            if (!vouching) return 'unchecked';
            if (token === null) return 'unverified';
            const entry = issued.get(token);
            return entry?.device === toBase64Url(device) && entry.username === username ? 'verified' : 'invalid';
        },
    });
    return { issue, voucher };
};

type Realm = ReturnType<typeof createRealm>;

const party = async (
    id: SessionId,
    username: string,
    realm: Realm,
    certify: (device: DeviceKey) => string | null = (device) => realm.issue(device, username),
): Promise<Party> => {
    const commands: WorkerCommand[] = [];
    const trust = new Map<SessionId, DeviceTrust>();
    const outbox: Party['outbox'] = [];
    const device = await createDeviceKey();
    const session = await startE2eeSession({
        selfId: id,
        device,
        voucher: realm.voucher(certify(device)),
        post: (command) => commands.push(command),
        send: (to, payload) => outbox.push({ to, payload }),
        onTrust: (peer, value) => trust.set(peer, value),
    });
    return { id, session, commands, trust, outbox };
};

/** Lets WebCrypto, which completes off the JavaScript thread, finish what is in flight. */
const flush = async () => {
    const until = performance.now() + 15;
    while (performance.now() < until) await new Promise((resolve) => setImmediate(resolve));
};

/** Delivers every message until nobody has anything left to say, as the server relay would. */
const settle = async (parties: Party[]) => {
    let quiet = 0;
    for (let round = 0; round < 60 && quiet < 3; round += 1) {
        await flush();
        const pending = parties.flatMap((from) => from.outbox.splice(0).map((message) => ({ from, ...message })));
        quiet = pending.length === 0 ? quiet + 1 : 0;
        for (const { from, to, payload } of pending) {
            await parties.find((p) => p.id === to)?.session.handle(from.id, payload);
        }
    }
};

const sendingKey = (p: Party) => p.commands.filter((c) => c.type === 'send-key').at(-1);

const receivedFrom = (p: Party, sender: SessionId) =>
    p.commands.filter(
        (c): c is Extract<WorkerCommand, { type: 'receive-key' }> => c.type === 'receive-key' && c.sender === sender,
    );

/** Whether `receiver` can decrypt a frame `sender` encrypts now. */
const canDecrypt = async (sender: Party, receiver: Party): Promise<boolean> => {
    const key = sendingKey(sender);
    if (key?.type !== 'send-key') return false;
    const sealed = await encryptFrame(new Uint8Array([1, 2, 3, 4, 5, 6]).buffer, 'audio', undefined, {
        ...key,
        counter: 0,
    });
    const keys = (keyId: number) =>
        receivedFrom(receiver, sender.id)
            .filter((c) => c.keyId === keyId)
            .map((c) => c.key);
    return (await decryptFrame(sealed, 'audio', undefined, keys)) !== null;
};

describe('the end-to-end key exchange', () => {
    beforeEach(() => {
        // Only the key switch timer is faked; WebCrypto needs real turns of the event loop.
        vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    });

    afterEach(() => {
        vi.useRealTimers();
    });

    /** Two participants who know each other as `alice` (a) and `bob` (b). */
    const meet = async (alice: Party, bob: Party) => {
        alice.session.addPeer(bob.id, 'bob', false);
        bob.session.addPeer(alice.id, 'alice', true);
        await settle([alice, bob]);
        await vi.advanceTimersByTimeAsync(KEY_SWITCH_DELAY_MS);
    };

    test('two devices the realm vouches for exchange media keys and decrypt each other', async () => {
        const realm = createRealm();
        const alice = await party('a', 'alice', realm);
        const bob = await party('b', 'bob', realm);
        await meet(alice, bob);
        expect(await canDecrypt(alice, bob)).toBe(true);
        expect(await canDecrypt(bob, alice)).toBe(true);
        expect(alice.trust.get('b')).toBe('verified');
        expect(bob.trust.get('a')).toBe('verified');
        expect(alice.trust.get('a')).toBe('verified');
    });

    test('with a realm, a device without a certificate is unverified and gets no media key', async () => {
        const realm = createRealm();
        const alice = await party('a', 'alice', realm);
        const bob = await party('b', 'bob', realm, () => null);
        await meet(alice, bob);
        expect(alice.trust.get('b')).toBe('unverified');
        expect(bob.trust.get('b')).toBe('unverified');
        expect(await canDecrypt(alice, bob)).toBe(false);
        expect(await canDecrypt(bob, alice)).toBe(false);
    });

    test('a certificate for another device or another name is invalid: a swapped key gets nothing', async () => {
        const realm = createRealm();
        const alice = await party('a', 'alice', realm);
        const real = await createDeviceKey();
        // The server's own device, presenting the certificate of bob's real device.
        const impostor = await party('b', 'bob', realm, () => realm.issue(real, 'bob'));
        await meet(alice, impostor);
        expect(alice.trust.get('b')).toBe('invalid');
        expect(await canDecrypt(alice, impostor)).toBe(false);

        const carol = await party('c', 'carol', realm, (device) => realm.issue(device, 'mallory'));
        alice.session.addPeer('c', 'carol', true);
        carol.session.addPeer('a', 'alice', false);
        await settle([alice, carol]);
        expect(alice.trust.get('c')).toBe('invalid');
    });

    test('without a realm, keys flow and every device is unchecked, this one included', async () => {
        const realm = createRealm(false);
        const alice = await party('a', 'alice', realm);
        const bob = await party('b', 'bob', realm);
        await meet(alice, bob);
        expect(alice.trust.get('b')).toBe('unchecked');
        expect(alice.trust.get('a')).toBe('unchecked');
        expect(await canDecrypt(alice, bob)).toBe(true);
    });

    test('a hello signed for someone else, or with another call key, is invalid and gets no media key', async () => {
        const realm = createRealm();
        const alice = await party('a', 'alice', realm);
        const bob = await party('b', 'bob', realm);
        alice.session.addPeer('b', 'bob', false);
        bob.session.addPeer('a', 'alice', false);
        await flush();
        const hello = bob.outbox.splice(0).find((m) => parseKeyMessage(m.payload)?.type === 'hello');
        const parsed = hello && parseKeyMessage(hello.payload);
        if (parsed?.type !== 'hello') throw new Error('no hello');
        const swapped = encodeKeyMessage({ ...parsed, ephemeral: new Uint8Array(65).fill(4) });
        await alice.session.handle('b', swapped);
        await flush();
        expect(alice.trust.get('b')).toBe('invalid');
        expect(alice.outbox.filter((m) => parseKeyMessage(m.payload)?.type === 'key')).toEqual([]);
    });

    test('after someone leaves, the media key changes and the leaver never receives the new one', async () => {
        const realm = createRealm();
        const [a, b, c] = await Promise.all([party('a', 'a', realm), party('b', 'b', realm), party('c', 'c', realm)]);
        for (const [self, others] of [
            [a, [b, c]],
            [b, [a, c]],
            [c, [a, b]],
        ] as const) {
            for (const other of others) self.session.addPeer(other.id, other.id, false);
        }
        await settle([a, b, c]);
        const before = sendingKey(a);
        a.session.removePeer('c');
        await settle([a, b, c]);
        await vi.advanceTimersByTimeAsync(KEY_SWITCH_DELAY_MS);
        expect(sendingKey(a)).not.toEqual(before);
        expect(await canDecrypt(a, b)).toBe(true);
        expect(await canDecrypt(a, c)).toBe(false);
        expect(a.commands).toContainEqual({ type: 'forget', sender: 'c', keyIds: null });
    });

    test('a lost key message is recovered: the receiver says hello again and the peer resends its keys', async () => {
        const realm = createRealm();
        const alice = await party('a', 'alice', realm);
        const bob = await party('b', 'bob', realm);
        alice.session.addPeer('b', 'bob', false);
        bob.session.addPeer('a', 'alice', false);
        // Everything bob sends is lost once, as a relay that drops messages would.
        await flush();
        bob.outbox.splice(0);
        await settle([alice, bob]);
        expect(await canDecrypt(bob, alice)).toBe(false);
        await vi.advanceTimersByTimeAsync(HELLO_RETRY_MS);
        await settle([alice, bob]);
        expect(await canDecrypt(bob, alice)).toBe(true);
    });

    test('garbage from the relay is ignored', async () => {
        const alice = await party('a', 'alice', createRealm());
        alice.session.addPeer('b', 'bob', false);
        await alice.session.handle('b', 'not a message');
        await alice.session.handle('stranger', 'eyJ2IjoxfQ');
        expect(alice.trust.get('b')).toBe('pending');
    });
});
