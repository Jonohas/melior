import type { ChatAttachmentWire, OutgoingAttachment } from '@/features/chat/domain';
import type { ChannelId } from './channel-id';
import type { SessionId } from './session-id';
import type { VoiceState } from './voice-state';

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
    /** Mute, camera, screen and speaking, as the server last relayed them. */
    voice: VoiceState;
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
    /** Images or a GIF, in order, checked by the chat domain's parser. */
    attachments?: ChatAttachmentWire[];
};

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
    | { channel: 'voice_state'; payload: VoiceState & { session_id: SessionId } }
    | { channel: 'chat'; payload: ChatPayload }
    | { channel: 'chat_error'; payload: { message: string; client_id?: string } }
    | { channel: 'error'; payload: { message: string } }
    | { channel: 'route'; payload: { peer: SessionId; path: Path; epoch: number } }
    | { channel: 'link_close'; payload: { peer: SessionId } }
    | { channel: 'uplink'; payload: { active: boolean } }
    | { channel: 'peer_offer'; payload: { from: SessionId; sdp: string } }
    | { channel: 'peer_answer'; payload: { from: SessionId; sdp: string } }
    | { channel: 'peer_ice'; payload: { from: SessionId; candidate: IceCandidate } }
    /** An end-to-end key message from `from`, relayed unread by the server. */
    | { channel: 'e2ee'; payload: { from: SessionId; payload: string } };

export type ClientMessage =
    | { channel: 'sdp_offer'; payload: SessionDescription<'offer'> }
    | { channel: 'sdp_answer'; payload: SessionDescription<'answer'> }
    | { channel: 'ice'; payload: IceCandidate }
    | { channel: 'route_live'; payload: { peer: SessionId; epoch: number } }
    | { channel: 'link_failed'; payload: { peer: SessionId } }
    | { channel: 'peer_offer'; payload: { to: SessionId; sdp: string } }
    | { channel: 'peer_answer'; payload: { to: SessionId; sdp: string } }
    | { channel: 'peer_ice'; payload: { to: SessionId; candidate: IceCandidate } }
    | { channel: 'e2ee'; payload: { to: SessionId; payload: string } }
    | { channel: 'chat_send'; payload: { text: string; client_id?: string; attachments?: OutgoingAttachment[] } }
    | { channel: 'voice_state'; payload: VoiceState };
