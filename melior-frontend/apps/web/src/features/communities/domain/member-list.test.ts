import { describe, expect, test } from 'vitest';
import type { Role } from './community';
import type { Member } from './member';
import {
    groupMembers,
    highestRole,
    initialMemberListOpen,
    nameColor,
    parseMemberListOpen,
    usersInVoice,
} from './member-list';
import { COMMUNITY_ID, role } from './test-fixtures';
import { rosterOf } from './voice-roster';

const member = (username: string, roleIds: string[] = []): Member => ({ userId: `u-${username}`, username, roleIds });
const everyone = role(COMMUNITY_ID, 0);
const admin: Role = { ...role('r-admin', 9), name: 'Admin', color: 0xff0000 };
const mod: Role = { ...role('r-mod', 5), name: 'Mod', color: null };
const helper: Role = { ...role('r-helper', 2), name: 'Helper', color: 0x00ff00 };
const roles = [everyone, helper, admin, mod];

describe('groupMembers', () => {
    test('groups by highest role, highest first, members without a role last', () => {
        const groups = groupMembers(
            [
                member('zoe'),
                member('bob', ['r-helper', 'r-admin']),
                member('amy', ['r-mod']),
                member('carl', ['r-helper']),
            ],
            roles,
        );
        expect(groups.map((group) => [group.role?.name ?? 'Members', group.members.map((m) => m.username)])).toEqual([
            ['Admin', ['bob']],
            ['Mod', ['amy']],
            ['Helper', ['carl']],
            ['Members', ['zoe']],
        ]);
    });

    test('sorts by username within a group and leaves out empty groups', () => {
        const groups = groupMembers(
            [member('mia', ['r-mod']), member('ann', ['r-mod']), member('Bea', ['r-mod'])],
            roles,
        );
        expect(groups).toHaveLength(1);
        expect(groups[0]?.members.map((m) => m.username)).toEqual(['ann', 'Bea', 'mia']);
    });

    test('a role the view does not know counts as none', () => {
        expect(groupMembers([member('ann', ['r-gone'])], roles)[0]?.role).toBeNull();
    });

    test('nobody gives no groups', () => {
        expect(groupMembers([], roles)).toEqual([]);
    });
});

describe('highestRole and nameColor', () => {
    test('the highest role names the group; the highest coloured one colours the name', () => {
        const ann = member('ann', ['r-mod', 'r-helper']);
        expect(highestRole(ann, roles)?.id).toBe('r-mod');
        expect(nameColor(ann, roles)).toBe(0x00ff00);
        expect(nameColor(member('bob', ['r-admin', 'r-helper']), roles)).toBe(0xff0000);
        expect(nameColor(member('zoe'), roles)).toBeNull();
        expect(highestRole(member('zoe'), roles)).toBeNull();
    });
});

describe('usersInVoice', () => {
    test('collects the users of every voice channel', () => {
        const voice = { muted: false, camera: false, screen: false, speaking: false };
        const roster = rosterOf([
            { ...voice, channelId: 'v1', sessionId: 's1', userId: 'u-ann', username: 'ann' },
            { ...voice, channelId: 'v2', sessionId: 's2', userId: 'u-bob', username: 'bob' },
        ]);
        expect([...usersInVoice(roster)].sort()).toEqual(['u-ann', 'u-bob']);
    });
});

describe('member list open state', () => {
    test('reads only a stored true or false', () => {
        expect(parseMemberListOpen('true')).toBe(true);
        expect(parseMemberListOpen('false')).toBe(false);
        expect(parseMemberListOpen(null)).toBeNull();
        expect(parseMemberListOpen('yes')).toBeNull();
    });

    test('starts as left, else open only on a wide screen', () => {
        expect(initialMemberListOpen(false, true)).toBe(false);
        expect(initialMemberListOpen(true, false)).toBe(true);
        expect(initialMemberListOpen(null, true)).toBe(true);
        expect(initialMemberListOpen(null, false)).toBe(false);
    });
});
