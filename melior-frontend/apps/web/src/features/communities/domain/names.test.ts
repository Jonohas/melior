import { describe, expect, test } from 'vitest';
import { checkName, checkTopic } from './names';

describe('checkName', () => {
    test('trims and requires 1 to 100 characters', () => {
        expect(checkName('  Mods ')).toEqual({ ok: true, value: 'Mods' });
        expect(checkName('   ')).toMatchObject({ ok: false });
        expect(checkName('a'.repeat(100))).toMatchObject({ ok: true });
        expect(checkName('a'.repeat(101))).toMatchObject({ ok: false });
        expect(checkName('😀'.repeat(100))).toMatchObject({ ok: true });
    });
});

describe('checkTopic', () => {
    test('may be empty, at most 1024 characters', () => {
        expect(checkTopic('')).toEqual({ ok: true, value: '' });
        expect(checkTopic('a'.repeat(1025))).toMatchObject({ ok: false });
    });
});
