import { describe, expect, test } from 'vitest';
import { hasPermission, parsePermissions, togglePermission } from './permissions';

describe('parsePermissions', () => {
    test('keeps known names in canonical order and drops unknown ones', () => {
        expect(parsePermissions(['connect', 'view_channel', 'fly'])).toEqual(['view_channel', 'connect']);
        expect(parsePermissions([])).toEqual([]);
    });

    test('rejects anything that is not a list of strings', () => {
        expect(parsePermissions('connect')).toBeNull();
        expect(parsePermissions([1])).toBeNull();
        expect(parsePermissions(undefined)).toBeNull();
    });
});

describe('hasPermission', () => {
    test('administrator grants everything', () => {
        expect(hasPermission(['connect'], 'connect')).toBe(true);
        expect(hasPermission(['connect'], 'manage_roles')).toBe(false);
        expect(hasPermission(['administrator'], 'manage_roles')).toBe(true);
    });
});

describe('togglePermission', () => {
    test('adds and removes one bit', () => {
        expect(togglePermission(['connect'], 'view_channel', true)).toEqual(['view_channel', 'connect']);
        expect(togglePermission(['view_channel', 'connect'], 'connect', false)).toEqual(['view_channel']);
        expect(togglePermission([], 'connect', false)).toEqual([]);
    });
});
