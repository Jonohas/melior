import {
    type ChannelId,
    type ClientMessage,
    parseServerMessage,
    type ServerMessage,
} from '@/features/conference/domain';
import { sameOriginSocketUrl } from '@/lib/socket-url';

type SignallingClientOptions = {
    channelId: ChannelId;
    /** Called one message at a time: the next message is delivered only after the returned promise settles. */
    onMessage: (message: ServerMessage) => void | Promise<void>;
    /** `opened` is false when the socket closed before it ever opened, e.g. the upgrade was refused (401/403). */
    onClose: (info: { opened: boolean }) => void;
};

export type SignallingClient = {
    /** Returns false when the socket is not open, so nothing was sent. */
    send: (message: ClientMessage) => boolean;
    close: () => void;
};

export const createSignallingClient = ({
    channelId,
    onMessage,
    onClose,
}: SignallingClientOptions): SignallingClient => {
    const socket = new WebSocket(sameOriginSocketUrl(`/ws?channel_id=${encodeURIComponent(channelId)}`));
    let chain: Promise<void> = Promise.resolve();
    let opened = false;

    socket.onopen = () => {
        opened = true;
    };
    socket.onerror = (event) => console.error('WebSocket error', event);
    socket.onclose = () => onClose({ opened });
    socket.onmessage = (event) => {
        const message = typeof event.data === 'string' ? parseServerMessage(event.data) : null;
        if (!message) {
            console.warn('ignoring invalid or unknown server message');
            return;
        }
        chain = chain
            .then(() => onMessage(message))
            .catch((error) => {
                console.error(`error handling "${message.channel}"`, error);
            });
    };

    return {
        send: (message) => {
            if (socket.readyState !== WebSocket.OPEN) return false;
            socket.send(JSON.stringify(message));
            return true;
        },
        close: () => {
            socket.onopen = socket.onclose = socket.onerror = socket.onmessage = null;
            socket.close();
        },
    };
};
