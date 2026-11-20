import { parseScreenState, screenStateMessage } from '@/features/conference/domain';

type ScreenChannelOptions = {
    /** Whether this side shares its screen right now; read whenever the state is sent. */
    isSharing: () => boolean;
    /** The peer says whether it shares its screen. */
    onPeerScreen: (sharing: boolean) => void;
};

export type ScreenChannel = {
    /** Tells the peer whether this side shares its screen; does nothing until the channel is open. */
    announce: () => void;
    close: () => void;
};

/**
 * The negotiated data channel every direct link has, created by both sides before negotiation with the same
 * label and id. It carries one thing: whether each side shares its screen. Messages that do not parse are ignored.
 */
export const createScreenChannel = (
    connection: RTCPeerConnection,
    { isSharing, onPeerScreen }: ScreenChannelOptions,
): ScreenChannel => {
    const channel = connection.createDataChannel('melior', { negotiated: true, id: 0 });

    const announce = () => {
        if (channel.readyState !== 'open') return;
        try {
            channel.send(screenStateMessage(isSharing()));
        } catch (error) {
            console.warn('direct link: could not send the screen state', error);
        }
    };

    channel.onopen = announce;
    channel.onmessage = (event) => {
        const sharing = parseScreenState(event.data);
        if (sharing !== null) onPeerScreen(sharing);
    };

    return {
        announce,
        close: () => {
            channel.onopen = null;
            channel.onmessage = null;
            channel.close();
        },
    };
};
