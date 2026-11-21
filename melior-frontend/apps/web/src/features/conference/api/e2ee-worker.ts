import type { FrameMedia, FrameType } from '@/features/conference/domain';
import { decryptFrame, encryptFrame, type SendingKey } from './frame-cipher';

/**
 * The worker every `RTCRtpScriptTransform` of a call runs in. It encrypts each outgoing encoded frame with this
 * participant's media key and decrypts each incoming one with the key its trailer names. A frame it cannot
 * encrypt or decrypt is dropped, never passed on in the clear. One worker per call, so a media key has one
 * frame counter.
 */

export type TransformOptions = { operation: 'encrypt' | 'decrypt'; media: FrameMedia };

export type WorkerCommand =
    | { type: 'send-key'; keyId: number; key: CryptoKey }
    | { type: 'receive-key'; sender: string; keyId: number; key: CryptoKey }
    | { type: 'forget'; sender: string; keyIds: number[] | null };

type EncodedFrame = { data: ArrayBuffer; type?: FrameType };

type ScriptTransformer = {
    readable: ReadableStream<EncodedFrame>;
    writable: WritableStream<EncodedFrame>;
    options: TransformOptions;
};

type TransformEvent = Event & { transformer: ScriptTransformer };

/** The worker's global scope, as far as this file uses it; the project compiles against the DOM library. */
const scope = self as unknown as {
    onmessage: ((event: MessageEvent<WorkerCommand>) => void) | null;
    onrtctransform: ((event: TransformEvent) => void) | null;
};

let sending: SendingKey | null = null;
/** Key id → sender → key. */
const receiving = new Map<number, Map<string, CryptoKey>>();

const lookup = (keyId: number): CryptoKey[] => [...(receiving.get(keyId)?.values() ?? [])];

const forget = (sender: string, keyIds: number[] | null) => {
    for (const [keyId, senders] of receiving) {
        if (keyIds !== null && !keyIds.includes(keyId)) continue;
        senders.delete(sender);
        if (senders.size === 0) receiving.delete(keyId);
    }
};

scope.onmessage = (event: MessageEvent<WorkerCommand>) => {
    const command = event.data;
    switch (command.type) {
        case 'send-key':
            sending = { keyId: command.keyId, key: command.key, counter: 0 };
            return;
        case 'receive-key': {
            const senders = receiving.get(command.keyId) ?? new Map<string, CryptoKey>();
            senders.set(command.sender, command.key);
            receiving.set(command.keyId, senders);
            return;
        }
        case 'forget':
            forget(command.sender, command.keyIds);
            return;
    }
};

const transformFor = ({ operation, media }: TransformOptions) =>
    new TransformStream<EncodedFrame, EncodedFrame>({
        transform: async (frame, controller) => {
            const type = media === 'video' ? frame.type : undefined;
            if (operation === 'encrypt') {
                if (!sending) return;
                frame.data = await encryptFrame(frame.data, media, type, sending);
                controller.enqueue(frame);
                return;
            }
            const opened = await decryptFrame(frame.data, media, type, lookup);
            if (!opened) return;
            frame.data = opened;
            controller.enqueue(frame);
        },
    });

scope.onrtctransform = (event) => {
    const { readable, writable, options } = event.transformer;
    readable
        .pipeThrough(transformFor(options))
        .pipeTo(writable)
        .catch(() => undefined);
};
