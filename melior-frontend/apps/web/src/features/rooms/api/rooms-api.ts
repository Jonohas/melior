import {
    parseRoom,
    parseRooms,
    type Room,
    RoomNotFoundError,
    RoomRejectedError,
    type RoomSettings,
    toRoomBody,
} from '@/features/rooms/domain';
import { ForbiddenError, UnauthorizedError } from '@/lib/auth';

/**
 * Maps the status codes shared by every room endpoint to typed errors. A 401 becomes an UnauthorizedError,
 * which the query client's caches run through the login gate (`auth.handleUnauthorized`).
 */
const assertOk = async (response: Response): Promise<void> => {
    if (response.ok) return;
    if (response.status === 401) throw new UnauthorizedError();
    if (response.status === 403) throw new ForbiddenError('You may not change or view this room.');
    if (response.status === 404) throw new RoomNotFoundError();
    if (response.status === 400) {
        const text = (await response.text().catch(() => '')).trim();
        throw new RoomRejectedError(text !== '' && text.length < 200 ? text : undefined);
    }
    throw new Error(`Room request failed with HTTP ${response.status}`);
};

const roomUrl = (channelId: string) => `/api/rooms/${encodeURIComponent(channelId)}`;

const readRoom = async (response: Response): Promise<Room> => {
    const room = parseRoom(await response.json());
    if (!room) throw new Error('Unexpected room response');
    return room;
};

/** Rooms the user owns or is invited to (admins: all rooms). */
export const listRooms = async (): Promise<Room[]> => {
    const response = await fetch('/api/rooms');
    await assertOk(response);
    const rooms = parseRooms(await response.json());
    if (!rooms) throw new Error('Unexpected rooms response');
    return rooms;
};

/** One room. 404 (not registered yet) throws RoomNotFoundError; 403 (restricted, not invited) a ForbiddenError. */
export const getRoom = async (channelId: string): Promise<Room> => {
    const response = await fetch(roomUrl(channelId));
    await assertOk(response);
    return readRoom(response);
};

/** Creates the room (the caller becomes owner) or, for the owner or an admin, changes its access. */
export const saveRoom = async (channelId: string, settings: RoomSettings): Promise<Room> => {
    const response = await fetch(roomUrl(channelId), {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(toRoomBody(settings)),
    });
    await assertOk(response);
    return readRoom(response);
};

/** Owner or admin only. */
export const deleteRoom = async (channelId: string): Promise<void> => {
    const response = await fetch(roomUrl(channelId), { method: 'DELETE' });
    await assertOk(response);
};

/**
 * Whether the user may join the room, for the conference's reconnect logic: 403 means "not invited",
 * 404 means nobody registered it yet (joining registers it, so access is fine), anything else is unknown.
 */
export const probeRoomAccess = async (channelId: string): Promise<'ok' | 'forbidden' | 'unknown'> => {
    try {
        await getRoom(channelId);
        return 'ok';
    } catch (error) {
        if (error instanceof ForbiddenError) return 'forbidden';
        if (error instanceof RoomNotFoundError) return 'ok';
        return 'unknown';
    }
};
