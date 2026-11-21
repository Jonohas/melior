import { DIRECT_SECTIONS, type DirectSection } from '@/features/conference/domain';

/** The tracks this side sends on a link: microphone, camera and, while sharing, the screen and its sound. */
export type LocalTracks = Record<DirectSection, MediaStreamTrack | null>;

/** The screen's tracks of `LocalTracks`. */
export type ScreenTracks = Pick<LocalTracks, 'screen' | 'screenAudio'>;

/** The senders of a link's four fixed sections: audio, camera video, screen video, screen audio. */
export type SectionSenders = Record<DirectSection, RTCRtpSender | null>;

export const NO_SENDERS: SectionSenders = { audio: null, video: null, screen: null, screenAudio: null };

/** The tracks a captured screen puts on the links; none while nothing is shared or the sound was left out. */
export const screenTracksOf = (screen: MediaStream | null): ScreenTracks => ({
    screen: screen?.getVideoTracks()[0] ?? null,
    screenAudio: screen?.getAudioTracks()[0] ?? null,
});

/**
 * The offerer's sections, in the order every link has them: audio, camera, screen, screen audio. Audio and camera come from
 * `addTrack` when there is a track; a section without a track is still created, so the layout never varies.
 */
export const addOfferedSections = (
    connection: RTCPeerConnection,
    tracks: LocalTracks,
    stream: MediaStream | null,
): SectionSenders => {
    const section = (kind: 'audio' | 'video'): RTCRtpSender => {
        const track = tracks[kind];
        return track && stream
            ? connection.addTrack(track, stream)
            : connection.addTransceiver(kind, { direction: 'sendrecv' }).sender;
    };
    const audio = section('audio');
    const video = section('video');
    const screen = connection.addTransceiver(tracks.screen ?? 'video', { direction: 'sendrecv' }).sender;
    const screenAudio = connection.addTransceiver(tracks.screenAudio ?? 'audio', { direction: 'sendrecv' }).sender;
    return { audio, video, screen, screenAudio };
};

/**
 * The answerer's sections, after the offer was applied: the first audio and first video transceiver carry the
 * microphone and the camera, the second ones the screen's sound and picture. All four send as well as receive.
 * The senders are stored in `senders` as they are attached. The local tracks may change while a
 * `replaceTrack` is awaited, so they are read again afterwards and any section that is behind is replaced
 * again, until every section carries the current track.
 */
export const attachAnsweredSections = async (
    connection: RTCPeerConnection,
    local: () => LocalTracks,
    senders: SectionSenders,
): Promise<void> => {
    const transceivers = connection.getTransceivers();
    const [audio, screenAudio] = transceivers.filter((t) => t.receiver.track.kind === 'audio');
    const [video, screen] = transceivers.filter((t) => t.receiver.track.kind === 'video');
    const found = { audio, video, screen, screenAudio };
    const attached: Partial<LocalTracks> = {};
    for (;;) {
        const wanted = local();
        const behind = DIRECT_SECTIONS.filter((kind) => found[kind] && attached[kind] !== wanted[kind]);
        if (behind.length === 0) return;
        for (const kind of behind) {
            const transceiver = found[kind];
            if (!transceiver) continue;
            transceiver.direction = 'sendrecv';
            attached[kind] = wanted[kind];
            senders[kind] = transceiver.sender;
            await transceiver.sender.replaceTrack(wanted[kind]);
        }
    }
};

/** Which fixed section a local microphone or camera track belongs to. */
export const sectionOf = (track: MediaStreamTrack): 'audio' | 'video' => (track.kind === 'audio' ? 'audio' : 'video');
