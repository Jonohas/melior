/**
 * How far this browser trusts the device behind a participant. Keycloak vouches for a device key with a signed,
 * DPoP-bound token the participant sends in its hello (see `e2ee-certificate.ts`):
 *
 * - `verified`: the realm vouches that this device belongs to the person the call shows. Media keys flow.
 * - `unverified`: the realm vouches for devices, and this one has no valid token: it gets no media key and sees
 *   nothing.
 * - `unchecked`: no realm vouches for devices on this server (`E2EE_ISSUER` is empty). Keys flow to every device,
 *   and the call is encrypted, but a server that swaps keys would not be caught.
 * - `invalid`: the hello's signature, or the token, does not hold, or the token names someone else. No media key.
 * - `pending`: no hello yet.
 */
export type DeviceTrust = 'verified' | 'unverified' | 'unchecked' | 'invalid' | 'pending';

/** Whether a participant of this trust may receive this browser's media key. */
export const mayReceiveKeys = (trust: DeviceTrust, vouching: boolean): boolean =>
    trust === 'verified' || trust === 'unchecked' || (trust === 'unverified' && !vouching);

/** How many media keys of one sender a receiver keeps: the current one and those before a rotation. */
export const KEYS_KEPT_PER_SENDER = 3;

/** How long a sender keeps encrypting with its old key after sending a new one, so receivers have it first. */
export const KEY_SWITCH_DELAY_MS = 1000;

/**
 * How long to wait for a peer's media key before saying hello again, and how often. A relay can drop a message
 * (a restart, a rate limit); a new hello makes the peer send its keys again.
 */
export const HELLO_RETRY_MS = 3000;
export const HELLO_RETRIES = 5;

/** The key ids to keep for a sender after `added` arrived, newest last, and those to forget. */
export const keepNewestKeys = (held: readonly number[], added: number): { keep: number[]; forget: number[] } => {
    const all = [...held.filter((keyId) => keyId !== added), added];
    const cut = Math.max(0, all.length - KEYS_KEPT_PER_SENDER);
    return { keep: all.slice(cut), forget: all.slice(0, cut) };
};

/**
 * What the room badge says about encryption. `attention` names participants whose device is not verified or whose
 * hello did not hold; `waiting` those whose keys have not arrived yet.
 */
export type EncryptionSummary =
    | { kind: 'alone' }
    | { kind: 'secure' }
    /** Encrypted, keys flowing, but this server does not verify devices. */
    | { kind: 'unchecked' }
    | { kind: 'waiting'; peers: string[] }
    | { kind: 'attention'; unverified: string[]; invalid: string[]; self: boolean };

/**
 * `self` is this browser's own trust: `unverified` when the realm gave it no token, so the others will not send
 * it their media keys (or, without a realm, cannot verify it).
 */
export const summarizeEncryption = (
    participants: Record<string, string>,
    trust: Record<string, DeviceTrust>,
    self: DeviceTrust = 'verified',
): EncryptionSummary => {
    const ids = Object.keys(participants);
    if (ids.length === 0) return { kind: 'alone' };
    const selfUnverified = self === 'unverified';
    const named = (wanted: DeviceTrust) =>
        ids.filter((id) => (trust[id] ?? 'pending') === wanted).map((id) => participants[id] ?? id);
    const invalid = named('invalid');
    const unverified = named('unverified');
    if (invalid.length > 0 || unverified.length > 0 || selfUnverified) {
        return { kind: 'attention', unverified, invalid, self: selfUnverified };
    }
    const pending = named('pending');
    if (pending.length > 0) return { kind: 'waiting', peers: pending };
    const unchecked = self === 'unchecked' || named('unchecked').length > 0;
    return unchecked ? { kind: 'unchecked' } : { kind: 'secure' };
};
