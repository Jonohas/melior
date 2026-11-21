import { describe, expect, test } from 'vitest';
import { abilitiesOf, canAssignRole, canEditRole, canKick, grantablePermissions, memberRank } from './abilities';
import { PERMISSIONS } from './permissions';
import { COMMUNITY_ID, role, view } from './test-fixtures';

const me = view().me;

describe('abilitiesOf', () => {
    test('follows the base permissions', () => {
        expect(abilitiesOf(me)).toMatchObject({
            manageRoles: true,
            kickMembers: true,
            manageChannels: false,
            viewInvites: false,
            renameServer: false,
            deleteServer: false,
            leaveServer: true,
            openServerSettings: true,
        });
    });

    test('the owner may do everything except leave', () => {
        const owner = abilitiesOf({ ...me, permissions: [], isOwner: true });
        expect(owner).toMatchObject({
            manageChannels: true,
            renameServer: true,
            deleteServer: true,
            leaveServer: false,
        });
    });

    test('administrator renames but does not delete', () => {
        const admin = abilitiesOf({ ...me, permissions: ['administrator'] });
        expect(admin).toMatchObject({
            renameServer: true,
            manageChannels: true,
            viewInvites: true,
            deleteServer: false,
        });
    });

    test('a plain member has nothing to manage', () => {
        expect(abilitiesOf({ ...me, permissions: ['view_channel'] }).openServerSettings).toBe(false);
    });
});

describe('role hierarchy', () => {
    test('roles below the rank are editable, others are not', () => {
        expect(canEditRole(me, role('r-low', 2))).toBe(true);
        expect(canEditRole(me, role('r-same', 5))).toBe(false);
        expect(canEditRole({ ...me, isOwner: true }, role('r-high', 8))).toBe(true);
        expect(canEditRole({ ...me, permissions: [] }, role('r-low', 2))).toBe(false);
    });

    test('@everyone is never assigned', () => {
        expect(canAssignRole(view(), role(COMMUNITY_ID, 0))).toBe(false);
        expect(canAssignRole(view(), role('r-low', 2))).toBe(true);
    });

    test('grantable permissions are your own unless owner or administrator', () => {
        expect(grantablePermissions(me)).toEqual(me.permissions);
        expect(grantablePermissions({ ...me, permissions: ['administrator'] })).toEqual(PERMISSIONS);
    });
});

describe('canKick', () => {
    const member = (userId: string, roleIds: string[]) => ({ userId, username: userId, roleIds });

    test('only members below the rank, never the owner or yourself', () => {
        expect(canKick(view(), member('u2', ['r-low']))).toBe(true);
        expect(canKick(view(), member('u2', ['r-high']))).toBe(false);
        expect(canKick(view(), member('u-owner', []))).toBe(false);
        expect(canKick(view(), member('u-me', []))).toBe(false);
        expect(canKick(view({ me: { ...me, permissions: [] } }), member('u2', []))).toBe(false);
    });

    test('memberRank is the highest role position', () => {
        expect(memberRank({ roleIds: ['r-low', 'r-high'] }, view().roles)).toBe(8);
        expect(memberRank({ roleIds: [] }, view().roles)).toBe(0);
    });
});
