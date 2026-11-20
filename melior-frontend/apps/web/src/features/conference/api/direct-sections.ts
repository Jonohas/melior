import type { MediaKind } from '@/features/conference/domain';

/** The tracks this side sends on a link: microphone, camera and, while sharing, the screen. */
export type LocalTracks = Record<MediaKind, MediaStreamTrack | null>;

/** The senders of a link's three fixed sections: audio, camera video, screen video. */
export type SectionSenders = Record<MediaKind, RTCRtpSender | null>;

export const NO_SENDERS: SectionSenders = { audio: null, video: null, screen: null };

/**
 * The offerer's sections, in the order every link has them: audio, camera, screen. Audio and camera come from
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
    return { audio, video, screen };
};

/**
 * The answerer's sections, after the offer was applied: the first audio and first video transceiver carry the
 * microphone and the camera, the second video transceiver the screen. All three send as well as receive.
 */
export const attachAnsweredSections = async (
    connection: RTCPeerConnection,
    tracks: LocalTracks,
): Promise<SectionSenders> => {
    const transceivers = connection.getTransceivers();
    const audio = transceivers.find((t) => t.receiver.track.kind === 'audio');
    const [video, screen] = transceivers.filter((t) => t.receiver.track.kind === 'video');
    const senders: SectionSenders = { ...NO_SENDERS };
    for (const [kind, transceiver] of [
        ['audio', audio],
        ['video', video],
        ['screen', screen],
    ] as const) {
        if (!transceiver) continue;
        transceiver.direction = 'sendrecv';
        await transceiver.sender.replaceTrack(tracks[kind]);
        senders[kind] = transceiver.sender;
    }
    return senders;
};
