import type { IceCandidate } from '@/features/conference/domain';

/** Holds remote ICE candidates that arrive before the remote description is set, then applies them in order. */
export type IceCandidateQueue = {
    add: (candidate: IceCandidate) => Promise<void>;
    /** Applies the queued candidates; call it right after `setRemoteDescription`. */
    flush: () => Promise<void>;
};

export const createIceCandidateQueue = (connection: RTCPeerConnection, label: string): IceCandidateQueue => {
    let queued: IceCandidate[] = [];

    const apply = async (candidate: IceCandidate) => {
        try {
            await connection.addIceCandidate(candidate);
        } catch (error) {
            console.warn(`${label} addIceCandidate failed`, error);
        }
    };

    return {
        add: async (candidate) => {
            if (!connection.remoteDescription) {
                queued.push(candidate);
                return;
            }
            await apply(candidate);
        },
        flush: async () => {
            const waiting = queued;
            queued = [];
            for (const candidate of waiting) await apply(candidate);
        },
    };
};
