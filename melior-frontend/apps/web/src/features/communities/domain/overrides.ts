import type { Category, Channel, OverrideTarget, PermissionOverride } from './community';
import { type Permission, togglePermission } from './permissions';

/** One bit of an override: explicitly allowed, explicitly denied, or left to the roles. */
export type OverrideChoice = 'allow' | 'inherit' | 'deny';

export const sameTarget = (a: OverrideTarget, b: OverrideTarget): boolean => a.kind === b.kind && a.id === b.id;

export const choiceOf = (override: PermissionOverride | undefined, permission: Permission): OverrideChoice => {
    if (override?.allow.includes(permission)) return 'allow';
    if (override?.deny.includes(permission)) return 'deny';
    return 'inherit';
};

/** Sets one bit of one target's override; the target gets an override if it had none. */
export const setChoice = (
    overrides: readonly PermissionOverride[],
    target: OverrideTarget,
    permission: Permission,
    choice: OverrideChoice,
): PermissionOverride[] => {
    const withTarget = addTarget(overrides, target);
    return withTarget.map((entry) =>
        sameTarget(entry.target, target)
            ? {
                  target,
                  allow: togglePermission(entry.allow, permission, choice === 'allow'),
                  deny: togglePermission(entry.deny, permission, choice === 'deny'),
              }
            : entry,
    );
};

/** Adds an empty override for a target that has none. */
export const addTarget = (overrides: readonly PermissionOverride[], target: OverrideTarget): PermissionOverride[] =>
    overrides.some((entry) => sameTarget(entry.target, target))
        ? [...overrides]
        : [...overrides, { target, allow: [], deny: [] }];

export const removeTarget = (overrides: readonly PermissionOverride[], target: OverrideTarget): PermissionOverride[] =>
    overrides.filter((entry) => !sameTarget(entry.target, target));

/** The overrides that apply to a channel: its category's while synced, otherwise its own. */
export const effectiveOverrides = (channel: Channel, categories: readonly Category[]): PermissionOverride[] => {
    const category = categories.find((entry) => entry.id === channel.categoryId);
    return channel.synced && category ? category.overrides : channel.overrides;
};

export type OverrideBody = {
    target: { kind: OverrideTarget['kind']; id: string };
    allow: Permission[];
    deny: Permission[];
};

export const toOverrideBody = (override: PermissionOverride): OverrideBody => ({
    target: { kind: override.target.kind, id: override.target.id },
    allow: [...override.allow],
    deny: [...override.deny],
});
