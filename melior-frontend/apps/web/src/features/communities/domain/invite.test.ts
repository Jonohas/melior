import { describe, expect, test } from 'vitest';
import {
    describeExpiry,
    describeUses,
    inviteCodeFromInput,
    parseInvite,
    parseInvitePreview,
    parseJoinResult,
    toInviteBody,
} from './invite';

const wire = {
    code: 'aB3dE5fG7h',
    community_id: 'c1',
    created_by: 'alice',
    expires_at_ms: 1_790_000_000_000,
    max_uses: 5,
    uses: 1,
};

describe('parseInvite', () => {
    test('maps the wire shape, with nulls for never and unlimited', () => {
        expect(parseInvite(wire)).toEqual({
            code: 'aB3dE5fG7h',
            communityId: 'c1',
            createdBy: 'alice',
            expiresAtMs: 1_790_000_000_000,
            maxUses: 5,
            uses: 1,
        });
        expect(parseInvite({ ...wire, expires_at_ms: null, max_uses: null })).toMatchObject({
            expiresAtMs: null,
            maxUses: null,
        });
    });

    test('rejects bad codes and fields', () => {
        expect(parseInvite({ ...wire, code: 'short' })).toBeNull();
        expect(parseInvite({ ...wire, uses: -1 })).toBeNull();
        expect(parseInvite({ ...wire, max_uses: '5' })).toBeNull();
    });
});

describe('parseInvitePreview and parseJoinResult', () => {
    test('maps the preview', () => {
        const preview = { code: wire.code, community: { id: 'c1', name: 'Melior' }, member_count: 3, usable: true };
        expect(parseInvitePreview(preview)).toEqual({
            code: wire.code,
            community: { id: 'c1', name: 'Melior' },
            memberCount: 3,
            usable: true,
        });
        expect(parseInvitePreview({ ...preview, community: null })).toBeNull();
        expect(parseJoinResult({ community_id: 'c1' })).toBe('c1');
        expect(parseJoinResult({})).toBeNull();
    });
});

describe('inviteCodeFromInput', () => {
    test('accepts a bare code or an invite link', () => {
        expect(inviteCodeFromInput(' aB3dE5fG7h ')).toBe('aB3dE5fG7h');
        expect(inviteCodeFromInput('https://melior.example/invite/aB3dE5fG7h')).toBe('aB3dE5fG7h');
        expect(inviteCodeFromInput('https://melior.example/invite/aB3dE5fG7h/')).toBe('aB3dE5fG7h');
    });

    test('rejects anything else', () => {
        expect(inviteCodeFromInput('')).toBeNull();
        expect(inviteCodeFromInput('aB3dE5fG7')).toBeNull();
        expect(inviteCodeFromInput('https://x/other/aB3dE5fG7h')).toBeNull();
        expect(inviteCodeFromInput('aB3dE5fG7h!')).toBeNull();
    });
});

describe('invite descriptions and body', () => {
    test('describes expiry and uses', () => {
        expect(describeExpiry(null, 0)).toBe('never expires');
        expect(describeExpiry(1000, 2000)).toBe('expired');
        expect(describeExpiry(30 * 60_000, 0)).toBe('expires in 30 min');
        expect(describeExpiry(6 * 3_600_000, 0)).toBe('expires in 6 h');
        expect(describeExpiry(7 * 86_400_000, 0)).toBe('expires in 7 days');
        expect(describeUses({ uses: 2, maxUses: null })).toBe('2 uses');
        expect(describeUses({ uses: 2, maxUses: 5 })).toBe('2 / 5 uses');
    });

    test('leaves never and unlimited out of the body', () => {
        expect(toInviteBody({ maxAgeSeconds: null, maxUses: null })).toEqual({});
        expect(toInviteBody({ maxAgeSeconds: 3600, maxUses: 5 })).toEqual({ max_age_seconds: 3600, max_uses: 5 });
    });
});
