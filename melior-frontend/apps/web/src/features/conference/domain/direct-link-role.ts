import type { SessionId } from './session-id';

/** The side whose session id sorts lower (plain string comparison) makes the offer, so offers never collide. */
export const isOfferer = (self: SessionId, peer: SessionId): boolean => self < peer;
