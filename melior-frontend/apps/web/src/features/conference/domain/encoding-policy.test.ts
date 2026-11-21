import { describe, expect, test } from 'vitest';
import { applySenderPolicy, preferredVideoCodecs, type SendParametersLike, senderPolicy } from './encoding-policy';

const codecs = (...mimeTypes: string[]) => mimeTypes.map((mimeType) => ({ mimeType }));

describe('preferredVideoCodecs', () => {
    test('keeps VP8 and the resilience entries, drops every other codec', () => {
        const list = codecs('video/VP9', 'video/H264', 'video/VP8', 'video/rtx', 'video/AV1', 'video/red', 'video/VP8');
        expect(preferredVideoCodecs(list).map((c) => c.mimeType)).toEqual([
            'video/VP8',
            'video/VP8',
            'video/rtx',
            'video/red',
        ]);
    });

    test('matches the mime type case-insensitively', () => {
        expect(preferredVideoCodecs(codecs('video/vp8', 'video/H264')).map((c) => c.mimeType)).toEqual(['video/vp8']);
    });

    test('a browser without VP8 keeps its own list rather than none', () => {
        const list = codecs('video/H264', 'video/rtx');
        expect(preferredVideoCodecs(list)).toEqual(list);
    });
});

describe('senderPolicy', () => {
    test('a camera on a direct link is left to congestion control, with no cap', () => {
        expect(senderPolicy('video', 'direct')).toEqual({ encodings: [] });
    });

    test('a camera towards the SFU goes in three simulcast layers, lowest first, none capped', () => {
        const { encodings } = senderPolicy('video', 'sfu');
        expect(encodings.map((e) => e.rid)).toEqual(['q', 'h', 'f']);
        expect(encodings.map((e) => e.scaleResolutionDownBy)).toEqual([4, 2, 1]);
        for (const encoding of encodings) {
            expect(encoding.maxBitrate).toBeUndefined();
            expect(encoding.maxFramerate).toBeUndefined();
        }
    });

    test('a screen keeps its resolution on both paths', () => {
        for (const path of ['sfu', 'direct'] as const) {
            expect(senderPolicy('screen', path).degradationPreference).toBe('maintain-resolution');
        }
    });

    test('audio keeps the browser defaults', () => {
        expect(senderPolicy('audio', 'sfu')).toEqual({ encodings: [] });
    });
});

describe('applySenderPolicy', () => {
    const parameters = (...encodings: SendParametersLike['encodings']): SendParametersLike => ({ encodings });

    test('writes nothing without a policy or an uplink state', () => {
        const before = parameters({ active: true });
        expect(applySenderPolicy(before, null, null)).toBe(false);
        expect(before).toEqual(parameters({ active: true }));
    });

    test('sets the uplink state on every encoding and leaves the rest alone', () => {
        const target = parameters({ active: true, maxBitrate: 5 }, { active: true });
        expect(applySenderPolicy(target, null, false)).toBe(true);
        expect(target.encodings).toEqual([{ active: false, maxBitrate: 5 }, { active: false }]);
    });

    test('matches encodings by position without rids, and keeps active as it is when no state is given', () => {
        const target = parameters({ active: false });
        applySenderPolicy(target, { encodings: [{ maxBitrate: 9 }] }, null);
        expect(target).toEqual({ encodings: [{ active: false, maxBitrate: 9 }] });
    });

    test('matches encodings by rid and never writes a rid', () => {
        const target = parameters({ rid: 'h', active: true }, { rid: 'q', active: true });
        applySenderPolicy(
            target,
            {
                encodings: [
                    { rid: 'q', maxBitrate: 100, scaleResolutionDownBy: 4 },
                    { rid: 'h', maxBitrate: 500, maxFramerate: 20 },
                ],
            },
            true,
        );
        expect(target.encodings).toEqual([
            { rid: 'h', active: true, maxBitrate: 500, maxFramerate: 20 },
            { rid: 'q', active: true, maxBitrate: 100, scaleResolutionDownBy: 4 },
        ]);
    });
});
