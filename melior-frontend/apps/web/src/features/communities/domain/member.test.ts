import { describe, expect, test } from 'vitest';
import { parseMembers } from './member';

describe('parseMembers', () => {
    test('maps and sorts by username', () => {
        expect(
            parseMembers([
                { user_id: 'u2', username: 'zed', role_ids: [] },
                { user_id: 'u1', username: 'amy', role_ids: ['r'] },
            ]),
        ).toEqual([
            { userId: 'u1', username: 'amy', roleIds: ['r'] },
            { userId: 'u2', username: 'zed', roleIds: [] },
        ]);
    });

    test('rejects a malformed member', () => {
        expect(parseMembers([{ user_id: 'u', username: 'a', role_ids: [1] }])).toBeNull();
        expect(parseMembers(null)).toBeNull();
    });
});
