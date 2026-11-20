import type { Dispatch } from 'react';
import { createMesh, type Mesh, type PeerSession, type SignallingClient } from '@/features/conference/api';
import type { ConferenceAction, IceServer, SessionId } from '@/features/conference/domain';

type StartMeshOptions = {
    selfId: SessionId;
    iceServers: IceServer[];
    forceRelay: boolean;
    localStream: MediaStream | null;
    /** The current SFU connection and socket; read at call time because both are replaced on reconnect. */
    peer: () => PeerSession | null;
    signalling: () => SignallingClient | null;
    dispatch: Dispatch<ConferenceAction<MediaStream>>;
    /** False once the connection this mesh belongs to was torn down; late callbacks then do nothing. */
    isCurrent: () => boolean;
};

/** Connects the mesh to the conference: its signals go to the socket, its results to the reducer. */
export const startMesh = ({
    selfId,
    iceServers,
    forceRelay,
    localStream,
    peer,
    signalling,
    dispatch,
    isCurrent,
}: StartMeshOptions): Mesh =>
    createMesh({
        selfId,
        iceServers,
        forceRelay,
        localStream,
        sfu: () => peer()?.statsSource ?? null,
        send: (message) => {
            if (isCurrent()) signalling()?.send(message);
        },
        onDirectMedia: (sessionId, stream) => {
            if (isCurrent()) dispatch({ type: 'direct_media_updated', sessionId, stream });
        },
        onShownPath: (sessionId, path) => {
            if (isCurrent()) dispatch({ type: 'shown_path_changed', sessionId, path });
        },
    });
