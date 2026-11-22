import type { FrameMedia } from '@/features/conference/domain';
import type { TransformOptions, WorkerCommand } from './e2ee-worker';

/** Puts the encryption transforms on every sender and receiver of a connection that has none yet. */
export type Protect = (connection: RTCPeerConnection) => void;

export type FrameTransforms = {
    protect: Protect;
    /** Hands a key to the worker; `CryptoKey`s are cloned into it, non-extractable as they are. */
    post: (command: WorkerCommand) => void;
    close: () => void;
};

type ScriptTransformConstructor = new (worker: Worker, options: TransformOptions) => object;

type Transformable = { transform: object | null };

const scriptTransform = (): ScriptTransformConstructor | undefined =>
    (globalThis as { RTCRtpScriptTransform?: ScriptTransformConstructor }).RTCRtpScriptTransform;

/**
 * Whether this browser can encrypt media end to end. Chrome and Edge 141, Firefox 117 and Safari 15.4 can.
 * A browser that cannot does not join a call: one participant sending in the clear would end the guarantee.
 */
export const canEncryptMedia = (): boolean => scriptTransform() !== undefined;

/**
 * The worker of one call and the transforms that route every encoded frame through it. Each sender and
 * receiver gets its transform once; a transceiver keeps it while the server reuses the section for another
 * participant, because the worker picks the key from each frame's trailer.
 */
export const createFrameTransforms = (): FrameTransforms => {
    const Transform = scriptTransform();
    if (!Transform) throw new Error('This browser cannot encrypt media end to end.');
    const worker = new Worker(new URL('./e2ee-worker.ts', import.meta.url), { type: 'module', name: 'melior-e2ee' });
    const protectedEnds = new WeakSet<object>();

    const attach = (end: object, operation: TransformOptions['operation'], media: FrameMedia) => {
        if (protectedEnds.has(end)) return;
        protectedEnds.add(end);
        (end as Transformable).transform = new Transform(worker, { operation, media });
    };

    return {
        protect: (connection) => {
            for (const transceiver of connection.getTransceivers()) {
                const media: FrameMedia = transceiver.receiver.track.kind === 'audio' ? 'audio' : 'video';
                attach(transceiver.sender, 'encrypt', media);
                attach(transceiver.receiver, 'decrypt', media);
            }
        },
        post: (command) => worker.postMessage(command),
        close: () => worker.terminate(),
    };
};
