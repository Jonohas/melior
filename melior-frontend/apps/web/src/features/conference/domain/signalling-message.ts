import type { ChannelId } from './channel-id';
import type { SessionId } from './session-id';

export type IceServer = {
    urls: string[];
    username?: string;
    credential?: string;
};

export type IceCandidate = {
    candidate?: string;
    sdpMid?: string | null;
    sdpMLineIndex?: number | null;
    usernameFragment?: string | null;
};

export type SessionDescription<T extends 'offer' | 'answer'> = {
    type: T;
    sdp: string;
};

export type Participant = {
    session_id: SessionId;
    username: string;
};

/** What a published track shows: the participant's camera (and microphone) or its screen. */
export type TrackSource = 'camera' | 'screen';

export const TRACK_SOURCES: readonly TrackSource[] = ['camera', 'screen'];

/** One media section (by mid) that currently carries a participant's track. */
export type TrackAssignment = {
    mid: string;
    session_id: SessionId;
    source: TrackSource;
};

/** A server offer names, for every media section that carries a participant's track, who publishes it. */
export type ServerOffer = SessionDescription<'offer'> & { tracks: TrackAssignment[] };

/** A saved chat message as the server sends it to the room. */
export type ChatPayload = {
    /** Orders the room's messages; the server assigns it. */
    id: number;
    username: string;
    text: string;
    /** Milliseconds since the Unix epoch. */
    sent_at_ms: number;
    /** Set only on the author's own copy of a message it just sent. */
    client_id?: string;
};

/** Why the server closed the socket, sent in a `closed` message right before it does. */
export type CloseReason = 'session_ended' | 'removed' | 'room_moved';

export const CLOSE_REASONS: readonly CloseReason[] = ['session_ended', 'removed', 'room_moved'];

export type ServerMessage =
    | { channel: 'closed'; payload: { reason: CloseReason } }
    | {
          channel: 'session';
          payload: {
              session_id: SessionId;
              channel_id: ChannelId;
              username: string;
              participants: Participant[];
              ice_servers: IceServer[];
          };
      }
    | { channel: 'peer_joined'; payload: Participant }
    | { channel: 'sdp_answer'; payload: SessionDescription<'answer'> }
    | { channel: 'sdp_offer'; payload: ServerOffer }
    | { channel: 'ice'; payload: IceCandidate }
    | { channel: 'peer_left'; payload: { session_id: SessionId } }
    | { channel: 'chat'; payload: ChatPayload }
    | { channel: 'chat_error'; payload: { message: string; client_id?: string } }
    | { channel: 'error'; payload: { message: string } };

export type ClientMessage =
    | { channel: 'sdp_offer'; payload: SessionDescription<'offer'> }
    | { channel: 'sdp_answer'; payload: SessionDescription<'answer'> }
    | { channel: 'ice'; payload: IceCandidate }
    | { channel: 'chat_send'; payload: { text: string; client_id?: string } };
