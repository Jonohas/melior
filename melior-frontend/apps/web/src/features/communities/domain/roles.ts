import type { Role, RoleId } from './community';
import type { Permission } from './permissions';

/** Highest first, as the role list shows them. */
export const rolesByRank = (roles: readonly Role[]): Role[] => [...roles].sort((a, b) => b.position - a.position);

export type PositionUpdate = { roleId: RoleId; position: number };

/**
 * Moving a role one step up or down the list: swap positions with the neighbour (never `@everyone`, position 0).
 * Two roles on the same position are told apart by lifting the upper one by one. Empty when there is nowhere to go.
 */
export const moveRole = (roles: readonly Role[], roleId: RoleId, direction: 'up' | 'down'): PositionUpdate[] => {
    const ordered = rolesByRank(roles).filter((role) => role.position > 0);
    const index = ordered.findIndex((role) => role.id === roleId);
    const role = ordered[index];
    const neighbour = ordered[direction === 'up' ? index - 1 : index + 1];
    if (index < 0 || !role || !neighbour) return [];
    const upper = direction === 'up' ? role : neighbour;
    const lower = direction === 'up' ? neighbour : role;
    if (role.position === neighbour.position) return [{ roleId: upper.id, position: lower.position + 1 }];
    return [
        { roleId: role.id, position: neighbour.position },
        { roleId: neighbour.id, position: role.position },
    ];
};

/** `#rrggbb` for a stored colour. */
export const colorToHex = (color: number): string => `#${color.toString(16).padStart(6, '0')}`;

/** A colour from an `<input type="color">` (`#rrggbb`), or null when it is not one. */
export const parseHexColor = (raw: string): number | null =>
    /^#[0-9a-fA-F]{6}$/.test(raw) ? Number.parseInt(raw.slice(1), 16) : null;

export type RoleChange = { name?: string; color?: number | null; permissions?: Permission[]; position?: number };

/** The request body of creating a role (`POST`) or changing one (`PATCH`); absent fields stay as they are. */
export const toRoleBody = (change: RoleChange): RoleChange => ({
    ...(change.name === undefined ? {} : { name: change.name }),
    ...(change.color === undefined ? {} : { color: change.color }),
    ...(change.permissions === undefined ? {} : { permissions: change.permissions }),
    ...(change.position === undefined ? {} : { position: change.position }),
});
