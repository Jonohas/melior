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

/** One media section (by mid) that currently carries a participant's track. */
export type TrackAssignment = {
    mid: string;
    session_id: SessionId;
};

/** A server offer names, for every media section that carries a participant's track, who publishes it. */
export type ServerOffer = SessionDescription<'offer'> & { tracks: TrackAssignment[] };

/** Why the server closed the socket, sent in a `closed` message right before it does. */
export type CloseReason = 'session_ended' | 'removed' | 'room_moved';

export const CLOSE_REASONS: readonly CloseReason[] = ['session_ended', 'removed', 'room_moved'];

/** The two ways media reaches a participant: browser to browser, or through the SFU. */
export type Path = 'direct' | 'forwarded';

export const PATHS: readonly Path[] = ['direct', 'forwarded'];

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
    | { channel: 'error'; payload: { message: string } }
    | { channel: 'route'; payload: { peer: SessionId; path: Path; epoch: number } }
    | { channel: 'link_close'; payload: { peer: SessionId } }
    | { channel: 'uplink'; payload: { active: boolean } }
    | { channel: 'peer_offer'; payload: { from: SessionId; sdp: string } }
    | { channel: 'peer_answer'; payload: { from: SessionId; sdp: string } }
    | { channel: 'peer_ice'; payload: { from: SessionId; candidate: IceCandidate } };

export type ClientMessage =
    | { channel: 'sdp_offer'; payload: SessionDescription<'offer'> }
    | { channel: 'sdp_answer'; payload: SessionDescription<'answer'> }
    | { channel: 'ice'; payload: IceCandidate }
    | { channel: 'route_live'; payload: { peer: SessionId; epoch: number } }
    | { channel: 'peer_offer'; payload: { to: SessionId; sdp: string } }
    | { channel: 'peer_answer'; payload: { to: SessionId; sdp: string } }
    | { channel: 'peer_ice'; payload: { to: SessionId; candidate: IceCandidate } };
