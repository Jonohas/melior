import {
    type DeviceTrust,
    encodeKeyMessage,
    HELLO_RETRIES,
    HELLO_RETRY_MS,
    type Hello,
    KEY_SWITCH_DELAY_MS,
    type KeyMessage,
    keepNewestKeys,
    mayReceiveKeys,
    parseKeyMessage,
    type SessionId,
    type WrappedKey,
} from '@/features/conference/domain';
import {
    type CallKey,
    createCallKey,
    createMediaKey,
    type DeviceKey,
    derivePairKey,
    type MediaKey,
    signHello,
    unwrapMediaKey,
    verifyHello,
    wrapMediaKey,
} from './e2ee-crypto';
import type { Voucher } from './e2ee-vouch';
import type { WorkerCommand } from './e2ee-worker';

/** Key messages that wait for the sender's hello; more than this from one peer are dropped. */
const MAX_WAITING_KEYS = 4;
/** Hellos from senders the server has not announced yet; more than this are dropped. */
const MAX_EARLY_HELLOS = 32;

type E2eeSessionOptions = {
    selfId: SessionId;
    /** This browser's device key, kept across calls. */
    device: DeviceKey;
    /** The realm that vouches for device keys, or none. */
    voucher: Voucher;
    /** Hands keys to the frame worker. */
    post: (command: WorkerCommand) => void;
    /** Sends a key message to one participant through the server. */
    send: (to: SessionId, payload: string) => void;
    /**
     * How far a participant's device is trusted, each time that changes. Called once for `selfId` too: `verified`
     * when the realm gave this browser a token to show, `unverified` when it did not.
     */
    onTrust: (peer: SessionId, trust: DeviceTrust) => void;
};

/** The end-to-end key exchange of one call. */
export type E2eeSession = {
    /**
     * Someone to exchange keys with: everyone in `session` (`rotate` false: the media key is fresh), and every
     * `peer_joined` (`rotate` true: the newcomer must not decrypt what was sent before).
     */
    addPeer: (peer: SessionId, username: string, rotate: boolean) => void;
    /** A participant left: forget its keys, and replace the media key so it cannot decrypt what follows. */
    removePeer: (peer: SessionId) => void;
    /** A relayed `e2ee` message. Anything that does not parse or verify is dropped. */
    handle: (from: SessionId, payload: string) => Promise<void>;
    close: () => void;
};

type Peer = {
    username: string;
    pairKey: CryptoKey | null;
    keyIds: number[];
    waiting: WrappedKey[];
    retry: ReturnType<typeof setTimeout> | null;
};

type Own = {
    device: DeviceKey;
    /** The realm's token vouching for the device key, sent in every hello; null without one. */
    certificate: string | null;
    call: CallKey;
    current: MediaKey;
    next: MediaKey | null;
};

/**
 * Creates this call's keys, asks the realm to vouch for the device key, hands the first media key to the worker,
 * and returns the exchange. Every peer gets a signed hello with that vouching token; once a peer's hello verifies
 * and the realm vouches for its device, both derive a pair key and each sends the other its media key.
 */
export const startE2eeSession = async (options: E2eeSessionOptions): Promise<E2eeSession> => {
    const own: Own = {
        device: options.device,
        certificate: await options.voucher.ownCertificate(),
        call: await createCallKey(),
        current: await createMediaKey(),
        next: null,
    };
    options.post({ type: 'send-key', keyId: own.current.keyId, key: own.current.key });
    // The own entry tells the person whether the others can verify this device at all.
    options.onTrust(options.selfId, own.certificate ? 'verified' : 'unverified');
    return createExchange(options, own);
};

const createExchange = (options: E2eeSessionOptions, own: Own): E2eeSession => {
    const { selfId, voucher, post, send, onTrust } = options;
    const peers = new Map<SessionId, Peer>();
    /** Hellos that arrived before the server said who sent them. */
    const early = new Map<SessionId, Hello>();
    let switchTimer: ReturnType<typeof setTimeout> | null = null;
    let closed = false;

    const sendMessage = (to: SessionId, message: KeyMessage) => {
        if (!closed) send(to, encodeKeyMessage(message));
    };

    const sendHello = async (to: SessionId) => {
        const signature = await signHello(own.device, selfId, to, own.call.publicRaw);
        sendMessage(to, {
            type: 'hello',
            device: own.device.publicRaw,
            ephemeral: own.call.publicRaw,
            signature,
            certificate: own.certificate,
        });
    };

    const sendMediaKey = async (to: SessionId, pairKey: CryptoKey, media: MediaKey) =>
        sendMessage(to, await wrapMediaKey(pairKey, media, selfId, to));

    /** The keys a peer needs now: the current one and, during a switch, the next. */
    const sendOwnKeys = async (to: SessionId, pairKey: CryptoKey) => {
        await sendMediaKey(to, pairKey, own.current);
        if (own.next) await sendMediaKey(to, pairKey, own.next);
    };

    /** Says hello until the peer's first media key arrives, in case a message was lost on the way. */
    const greet = (to: SessionId, peer: Peer, retries: number) => {
        void sendHello(to);
        if (retries <= 0) return;
        peer.retry = setTimeout(() => {
            if (!closed && peers.get(to) === peer && peer.keyIds.length === 0) greet(to, peer, retries - 1);
        }, HELLO_RETRY_MS);
    };

    const forgetPeer = (peer: Peer) => {
        if (peer.retry) clearTimeout(peer.retry);
    };

    const switchTo = (next: MediaKey) => {
        if (closed || own.next !== next) return;
        own.current = next;
        own.next = null;
        post({ type: 'send-key', keyId: next.keyId, key: next.key });
    };

    /** Sends a new media key to every peer, and encrypts with it once they have had time to receive it. */
    const rotate = async () => {
        const next = await createMediaKey();
        own.next = next;
        await Promise.all(
            [...peers].flatMap(([id, peer]) => (peer.pairKey ? [sendMediaKey(id, peer.pairKey, next)] : [])),
        );
        if (switchTimer) clearTimeout(switchTimer);
        switchTimer = setTimeout(() => switchTo(next), KEY_SWITCH_DELAY_MS);
    };

    const acceptKey = async (from: SessionId, peer: Peer, message: WrappedKey) => {
        if (!peer.pairKey) {
            if (peer.waiting.length < MAX_WAITING_KEYS) peer.waiting.push(message);
            return;
        }
        const key = await unwrapMediaKey(peer.pairKey, message, from, selfId);
        if (!key || peers.get(from) !== peer) return;
        post({ type: 'receive-key', sender: from, keyId: message.keyId, key });
        const { keep, forget } = keepNewestKeys(peer.keyIds, message.keyId);
        peer.keyIds = keep;
        if (forget.length > 0) post({ type: 'forget', sender: from, keyIds: forget });
    };

    const acceptHello = async (from: SessionId, peer: Peer, hello: Hello) => {
        if (!(await verifyHello(hello.device, from, selfId, hello.ephemeral, hello.signature))) {
            onTrust(from, 'invalid');
            return;
        }
        const trust = await voucher.verify(hello.certificate, hello.device, peer.username);
        if (closed || peers.get(from) !== peer) return;
        onTrust(from, trust);
        // A device the realm does not vouch for gets no key, and its keys are not taken either.
        if (!mayReceiveKeys(trust, voucher.vouching)) return;
        const pairKey = await derivePairKey(own.call, hello.ephemeral, selfId, from);
        if (closed || peers.get(from) !== peer) return;
        peer.pairKey = pairKey;
        await sendOwnKeys(from, pairKey);
        for (const waiting of peer.waiting.splice(0)) await acceptKey(from, peer, waiting);
    };

    return {
        addPeer: (id, username, shouldRotate) => {
            if (peers.has(id) || id === selfId || closed) return;
            const peer: Peer = { username, pairKey: null, keyIds: [], waiting: [], retry: null };
            peers.set(id, peer);
            onTrust(id, 'pending');
            greet(id, peer, HELLO_RETRIES);
            const hello = early.get(id);
            early.delete(id);
            if (hello) void acceptHello(id, peer, hello);
            if (shouldRotate) void rotate();
        },
        removePeer: (id) => {
            early.delete(id);
            const peer = peers.get(id);
            if (!peer) return;
            forgetPeer(peer);
            peers.delete(id);
            post({ type: 'forget', sender: id, keyIds: null });
            void rotate();
        },
        handle: async (from, payload) => {
            const message = parseKeyMessage(payload);
            if (!message || closed) return;
            const peer = peers.get(from);
            if (message.type === 'hello') {
                if (peer) return acceptHello(from, peer, message);
                if (early.size < MAX_EARLY_HELLOS) early.set(from, message);
                return;
            }
            if (peer) return acceptKey(from, peer, message);
        },
        close: () => {
            closed = true;
            if (switchTimer) clearTimeout(switchTimer);
            for (const peer of peers.values()) forgetPeer(peer);
            peers.clear();
            early.clear();
        },
    };
};
