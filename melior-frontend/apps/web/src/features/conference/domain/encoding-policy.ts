import type { MediaKind } from './path-flow';

/**
 * How this client compresses what it sends: which codec, at what size and rate, and what the encoder gives up
 * first under load. Pure decisions; `api/` applies them to the browser's senders and tracks.
 */

/** The one video codec every section negotiates. It keeps one set of media sections per room on the SFU. */
export const VIDEO_CODEC = 'video/VP8';

/** Codec entries that only protect another codec's packets; they stay next to VP8 so loss recovery works. */
const RESILIENCE_CODECS = ['video/rtx', 'video/red', 'video/ulpfec', 'video/flexfec-03'];

/** Which connection a sender belongs to: the one to the SFU, or a direct link to one peer. */
export type SendPath = 'sfu' | 'direct';

/** The part of a codec capability the order depends on. */
export type CodecEntry = { mimeType: string };

const isMime = (entry: CodecEntry, mime: string): boolean => entry.mimeType.toLowerCase() === mime.toLowerCase();

/**
 * The codec list for `setCodecPreferences`: VP8 and the resilience entries, nothing else. A browser without VP8
 * keeps its own list, so a section is never left without a codec.
 */
export const preferredVideoCodecs = <T extends CodecEntry>(codecs: readonly T[]): T[] => {
    const vp8 = codecs.filter((entry) => isMime(entry, VIDEO_CODEC));
    if (vp8.length === 0) return [...codecs];
    const resilience = codecs.filter((entry) => RESILIENCE_CODECS.some((mime) => isMime(entry, mime)));
    return [...vp8, ...resilience];
};

/** What the encoder is told the track shows: it tunes for motion, fine detail or speech accordingly. */
export const CONTENT_HINTS: Record<MediaKind, string> = {
    audio: 'speech',
    video: 'motion',
    screen: 'detail',
};

/** One encoding a sender produces. `rid` names a simulcast layer; without it encodings match by position. */
export type EncodingPolicy = {
    rid?: string;
    maxBitrate?: number;
    scaleResolutionDownBy?: number;
    maxFramerate?: number;
};

export type DegradationPreference = 'balanced' | 'maintain-framerate' | 'maintain-resolution';

export type SenderPolicy = {
    degradationPreference?: DegradationPreference;
    encodings: EncodingPolicy[];
};

/**
 * The camera's layers towards the SFU, lowest first, named by `rid` as the server expects (`q`, `h`, `f`): a
 * quarter, half and the full size of what the camera captures. No layer has a bitrate or frame-rate cap; the
 * browser shares its bandwidth among them, and drops the top layers when its uplink is short.
 */
export const SIMULCAST_LAYERS: readonly EncodingPolicy[] = [
    { rid: 'q', scaleResolutionDownBy: 4 },
    { rid: 'h', scaleResolutionDownBy: 2 },
    { rid: 'f', scaleResolutionDownBy: 1 },
];

/** Leaves every setting to the browser. */
const BROWSER_DEFAULTS: SenderPolicy = { encodings: [] };

/**
 * The settings of one sender. Nothing caps quality here: how sharp and smooth the video is is the person's
 * choice (`video-quality.ts`), and congestion control fits it to the network. A direct link has exactly one
 * receiver, so its camera sends one stream that the browser adapts to that receiver's network. The SFU does not
 * transcode, so its camera goes in three simulcast layers and the SFU picks one per receiver. A screen that
 * runs short of bandwidth keeps its resolution and gives up frames, so text stays readable.
 */
export const senderPolicy = (kind: MediaKind, path: SendPath): SenderPolicy => {
    if (kind === 'screen') {
        return { degradationPreference: 'maintain-resolution', encodings: [] };
    }
    if (kind === 'video' && path === 'sfu') return { encodings: [...SIMULCAST_LAYERS] };
    return BROWSER_DEFAULTS;
};

/** The shape of `RTCRtpSendParameters` the policy touches. */
export type SendParametersLike = {
    encodings: Array<EncodingPolicy & { active?: boolean }>;
    degradationPreference?: string;
};

const encodingPolicyFor = (policy: SenderPolicy, index: number, rid: string | undefined): EncodingPolicy | undefined =>
    rid === undefined ? policy.encodings[index] : policy.encodings.find((encoding) => encoding.rid === rid);

/** The rate and size settings only: an encoding's `rid` is fixed at negotiation and never written. */
const copySettings = (from: EncodingPolicy, to: EncodingPolicy): void => {
    if (from.maxBitrate !== undefined) to.maxBitrate = from.maxBitrate;
    if (from.scaleResolutionDownBy !== undefined) to.scaleResolutionDownBy = from.scaleResolutionDownBy;
    if (from.maxFramerate !== undefined) to.maxFramerate = from.maxFramerate;
};

/**
 * Writes `policy` and the uplink state into `parameters`, in place, because the browser only accepts the
 * object its own `getParameters` returned. `active` null leaves every encoding's `active` as it is. Returns
 * whether anything was written.
 */
export const applySenderPolicy = (
    parameters: SendParametersLike,
    policy: SenderPolicy | null,
    active: boolean | null,
): boolean => {
    if (policy === null && active === null) return false;
    parameters.encodings.forEach((encoding, index) => {
        if (active !== null) encoding.active = active;
        const wanted = policy ? encodingPolicyFor(policy, index, encoding.rid) : undefined;
        if (wanted) copySettings(wanted, encoding);
    });
    if (policy?.degradationPreference) parameters.degradationPreference = policy.degradationPreference;
    return true;
};
