import {
    type ChannelId,
    type ClientMessage,
    parseServerMessage,
    type ServerMessage,
} from '@/features/conference/domain';

type SignallingClientOptions = {
    channelId: ChannelId;
    /** Called one message at a time: the next message is delivered only after the returned promise settles. */
    onMessage: (message: ServerMessage) => void | Promise<void>;
    /** `opened` is false when the socket closed before it ever opened, e.g. the upgrade was refused (401/403). */
    onClose: (info: { opened: boolean }) => void;
};

export type SignallingClient = {
    send: (message: ClientMessage) => void;
    close: () => void;
};

export const createSignallingClient = ({
    channelId,
    onMessage,
    onClose,
}: SignallingClientOptions): SignallingClient => {
    const scheme = window.location.protocol === 'https:' ? 'wss' : 'ws';
    const socket = new WebSocket(`${scheme}://${window.location.host}/ws?channel_id=${encodeURIComponent(channelId)}`);
    let chain: Promise<void> = Promise.resolve();
    let opened = false;

    socket.onopen = () => {
        opened = true;
        console.log('WebSocket open');
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
            if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify(message));
        },
        close: () => {
            socket.onopen = socket.onclose = socket.onerror = socket.onmessage = null;
            socket.close();
        },
    };
};
