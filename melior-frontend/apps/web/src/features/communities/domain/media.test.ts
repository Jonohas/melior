import { describe, expect, test } from 'vitest';
import { checkMediaSettings, describeMediaWarning, mediaSummary } from './media';

describe('checkMediaSettings', () => {
    test('an empty limit is none, otherwise 2 to 99', () => {
        expect(checkMediaSettings('auto', ' ')).toEqual({ ok: true, userLimit: null });
        expect(checkMediaSettings('server', '99')).toEqual({ ok: true, userLimit: 99 });
        expect(checkMediaSettings('server', '1')).toMatchObject({ ok: false });
        expect(checkMediaSettings('server', '100')).toMatchObject({ ok: false });
        expect(checkMediaSettings('server', '2.5')).toMatchObject({ ok: false });
        expect(checkMediaSettings('server', 'ten')).toMatchObject({ ok: false });
    });

    test('peer to peer holds at most eight', () => {
        expect(checkMediaSettings('peer_to_peer', '8')).toEqual({ ok: true, userLimit: 8 });
        expect(checkMediaSettings('peer_to_peer', '9')).toMatchObject({ ok: false });
        expect(checkMediaSettings('auto', '9')).toEqual({ ok: true, userLimit: 9 });
    });
});

describe('mediaSummary', () => {
    const voice = { kind: 'voice', effectiveMode: 'peer_to_peer', effectiveLimit: 4 } as const;

    test('shows mode and occupancy for voice channels', () => {
        expect(mediaSummary(voice, 3)).toBe('P2P · 3/4');
        expect(mediaSummary({ ...voice, effectiveMode: 'server', effectiveLimit: null }, 2)).toBe('Server · 2');
        expect(mediaSummary({ ...voice, effectiveMode: 'auto', effectiveLimit: null }, 0)).toBe('Auto');
        expect(mediaSummary({ ...voice, kind: 'text' }, 0)).toBeNull();
    });

    test('explains each warning', () => {
        expect(describeMediaWarning('mesh_off')).toContain('turned off');
        expect(describeMediaWarning('limit_clamped')).toContain('lower limit');
    });
});
