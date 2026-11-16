import { describe, expect, test } from 'vitest';
import { addMember, normalizeUsername, removeMember } from './usernames';

describe('normalizeUsername', () => {
    test('lower-cases and trims', () => {
        expect(normalizeUsername('  Bob ')).toBe('bob');
        expect(normalizeUsername('Ann.Lee@Example-Org_1')).toBe('ann.lee@example-org_1');
    });

    test('rejects empty, too long and illegal characters', () => {
        expect(normalizeUsername('')).toBeNull();
        expect(normalizeUsername('   ')).toBeNull();
        expect(normalizeUsername('a'.repeat(65))).toBeNull();
        expect(normalizeUsername('a'.repeat(64))).toBe('a'.repeat(64));
        expect(normalizeUsername('bob smith')).toBeNull();
        expect(normalizeUsername('bob/smith')).toBeNull();
        expect(normalizeUsername('böb')).toBeNull();
    });
});

describe('addMember', () => {
    test('adds a normalised username', () => {
        expect(addMember(['bob'], ' Carol ')).toEqual({ ok: true, members: ['bob', 'carol'] });
    });

    test('reports empty input, invalid names and duplicates', () => {
        expect(addMember([], '  ')).toMatchObject({ ok: false });
        expect(addMember([], 'no spaces')).toMatchObject({ ok: false });
        expect(addMember(['bob'], 'BOB')).toEqual({ ok: false, error: 'bob is already invited.' });
    });
});

describe('removeMember', () => {
    test('removes only the named member', () => {
        expect(removeMember(['a', 'b', 'c'], 'b')).toEqual(['a', 'c']);
        expect(removeMember(['a'], 'zzz')).toEqual(['a']);
    });
});
