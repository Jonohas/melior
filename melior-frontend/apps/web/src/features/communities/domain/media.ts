import type { ChannelKind } from './community';

export const MEDIA_MODES = ['auto', 'peer_to_peer', 'server'] as const;
export type MediaMode = (typeof MEDIA_MODES)[number];

/** Why the effective media plan differs from what the channel asks for. */
export const MEDIA_WARNINGS = ['mesh_off', 'limit_clamped'] as const;
export type MediaWarning = (typeof MEDIA_WARNINGS)[number];

export const USER_LIMIT_MIN = 2;
export const USER_LIMIT_MAX = 99;
/** The most participants a peer-to-peer channel can hold on any server configuration. */
export const MESH_LIMIT_MAX = 8;

export const MEDIA_MODE_LABEL: Record<MediaMode, string> = {
    auto: 'Auto',
    peer_to_peer: 'P2P',
    server: 'Server',
};

export const describeMediaWarning = (warning: MediaWarning): string => {
    switch (warning) {
        case 'mesh_off':
            return 'Peer-to-peer is turned off on this server, so media goes through the server.';
        case 'limit_clamped':
            return 'The user limit is above what peer-to-peer allows here, so the lower limit applies.';
    }
};

export type MediaSettingsCheck = { ok: true; userLimit: number | null } | { ok: false; error: string };

/**
 * Checks the media settings form: an empty limit means none, otherwise 2 to 99, and a peer-to-peer channel at
 * most `MESH_LIMIT_MAX`. The server applies its own mesh limit on top.
 */
export const checkMediaSettings = (mode: MediaMode, rawLimit: string): MediaSettingsCheck => {
    const trimmed = rawLimit.trim();
    if (trimmed === '') return { ok: true, userLimit: null };
    const limit = Number(trimmed);
    if (!Number.isInteger(limit) || limit < USER_LIMIT_MIN || limit > USER_LIMIT_MAX) {
        return { ok: false, error: `The user limit is a whole number from ${USER_LIMIT_MIN} to ${USER_LIMIT_MAX}.` };
    }
    if (mode === 'peer_to_peer' && limit > MESH_LIMIT_MAX) {
        return { ok: false, error: `A peer-to-peer channel holds at most ${MESH_LIMIT_MAX} people.` };
    }
    return { ok: true, userLimit: limit };
};

type MediaFacts = { kind: ChannelKind; effectiveMode: MediaMode; effectiveLimit: number | null };

/** The short label beside a voice channel, e.g. "P2P · 3/4" or "Server · 2". Null for a text channel. */
export const mediaSummary = (channel: MediaFacts, occupants: number): string | null => {
    if (channel.kind !== 'voice') return null;
    const mode = MEDIA_MODE_LABEL[channel.effectiveMode];
    if (channel.effectiveLimit !== null) return `${mode} · ${occupants}/${channel.effectiveLimit}`;
    return occupants > 0 ? `${mode} · ${occupants}` : mode;
};
