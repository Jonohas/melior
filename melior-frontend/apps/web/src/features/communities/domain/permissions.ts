import { isString } from './wire';

/** Every permission bit, by its wire name. */
export const PERMISSIONS = [
    'view_channel',
    'connect',
    'send_messages',
    'read_history',
    'create_invite',
    'manage_channels',
    'manage_roles',
    'kick_members',
    'administrator',
] as const;

export type Permission = (typeof PERMISSIONS)[number];

/** The bits a channel or category override can change: the ones resolved per channel. */
export const CHANNEL_PERMISSIONS: readonly Permission[] = ['view_channel', 'connect', 'send_messages', 'read_history'];

/**
 * A list of permission names. Names this client does not know are dropped (a newer server may have more), but
 * anything that is not a list of strings is rejected.
 */
export const parsePermissions = (raw: unknown): Permission[] | null => {
    if (!Array.isArray(raw) || !raw.every(isString)) return null;
    return PERMISSIONS.filter((permission) => raw.includes(permission));
};

/** Whether the set grants the permission; `administrator` grants everything. */
export const hasPermission = (granted: readonly Permission[], permission: Permission): boolean =>
    granted.includes(permission) || granted.includes('administrator');

/** Adds or removes one permission, keeping the canonical order. */
export const togglePermission = (granted: readonly Permission[], permission: Permission, on: boolean): Permission[] =>
    PERMISSIONS.filter((entry) => (entry === permission ? on : granted.includes(entry)));
