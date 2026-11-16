/** A conference room and who may join it, as `/api/rooms` reports it. */
export type Room = {
    channelId: string;
    /** Username of the owner. */
    owner: string;
    /** Invite only: only the owner, invited usernames and admins may join. */
    restricted: boolean;
    /** Invited usernames, lower-case. */
    members: string[];
    /** True for the owner and for admins: may change the room's access. */
    isOwner: boolean;
};

/** What the owner can change. */
export type RoomSettings = {
    restricted: boolean;
    members: string[];
};

const isObject = (value: unknown): value is Record<string, unknown> =>
    typeof value === 'object' && value !== null && !Array.isArray(value);

/** Validates one room object from the API. Returns null for anything that is not the documented shape. */
export const parseRoom = (raw: unknown): Room | null => {
    if (!isObject(raw)) return null;
    const { channel_id, owner, restricted, members, is_owner } = raw;
    if (typeof channel_id !== 'string' || typeof owner !== 'string') return null;
    if (typeof restricted !== 'boolean' || typeof is_owner !== 'boolean') return null;
    if (!Array.isArray(members) || !members.every((m) => typeof m === 'string')) return null;
    return { channelId: channel_id, owner, restricted, members, isOwner: is_owner };
};

/** Validates a list response; null when the body is not an array or any entry is malformed. */
export const parseRooms = (raw: unknown): Room[] | null => {
    if (!Array.isArray(raw)) return null;
    const rooms = raw.map(parseRoom);
    return rooms.every((room): room is Room => room !== null) ? rooms : null;
};

/** The request body of `PUT /api/rooms/{channel_id}`. */
export const toRoomBody = (settings: RoomSettings): { restricted: boolean; members: string[] } => ({
    restricted: settings.restricted,
    members: settings.members,
});
