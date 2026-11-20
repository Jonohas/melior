import { describe, expect, test } from 'vitest';
import { colorToHex, moveRole, parseHexColor, rolesByRank, toRoleBody } from './roles';
import { role } from './test-fixtures';

const roles = [role('everyone', 0), role('a', 3), role('b', 6), role('c', 9)];

describe('rolesByRank', () => {
    test('sorts highest first', () => {
        expect(rolesByRank(roles).map((r) => r.id)).toEqual(['c', 'b', 'a', 'everyone']);
    });
});

describe('moveRole', () => {
    test('swaps positions with the neighbour', () => {
        expect(moveRole(roles, 'a', 'up')).toEqual([
            { roleId: 'a', position: 6 },
            { roleId: 'b', position: 3 },
        ]);
        expect(moveRole(roles, 'b', 'down')).toEqual([
            { roleId: 'b', position: 3 },
            { roleId: 'a', position: 6 },
        ]);
    });

    test('never passes @everyone or the top, and ignores unknown roles', () => {
        expect(moveRole(roles, 'a', 'down')).toEqual([]);
        expect(moveRole(roles, 'c', 'up')).toEqual([]);
        expect(moveRole(roles, 'zzz', 'up')).toEqual([]);
    });

    test('separates two roles on the same position', () => {
        const tied = [role('x', 4), role('y', 4)];
        expect(moveRole(tied, 'y', 'up')).toEqual([{ roleId: 'y', position: 5 }]);
        expect(moveRole(tied, 'x', 'down')).toEqual([{ roleId: 'y', position: 5 }]);
    });
});

describe('colours', () => {
    test('round-trips through #rrggbb', () => {
        expect(colorToHex(0xff0000)).toBe('#ff0000');
        expect(colorToHex(0x00000a)).toBe('#00000a');
        expect(parseHexColor('#00ff00')).toBe(0x00ff00);
        expect(parseHexColor('green')).toBeNull();
    });
});

describe('toRoleBody', () => {
    test('sends only what is set', () => {
        expect(toRoleBody({ name: 'Mod' })).toEqual({ name: 'Mod' });
        expect(toRoleBody({ color: null, permissions: ['connect'], position: 2 })).toEqual({
            color: null,
            permissions: ['connect'],
            position: 2,
        });
    });
});
