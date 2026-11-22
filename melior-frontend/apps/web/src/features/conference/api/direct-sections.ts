import { DIRECT_SECTIONS, type DirectSection, SYNC_GROUP, type SyncGroup } from '@/features/conference/domain';
import { addAudioSection, protectAnsweredAudio } from './audio-sections';

/** The tracks this side sends on a link: microphone, camera and, while sharing, the screen and its sound. */
export type LocalTracks = Record<DirectSection, MediaStreamTrack | null>;

/** The screen's tracks of `LocalTracks`. */
export type ScreenTracks = Pick<LocalTracks, 'screen' | 'screenAudio'>;

/** The senders of a link's four fixed sections: audio, camera video, screen video, screen audio. */
export type SectionSenders = Record<DirectSection, RTCRtpSender | null>;

export const NO_SENDERS: SectionSenders = { audio: null, video: null, screen: null, screenAudio: null };

/** The stream each sync group's sections announce to the peer, so it plays them in step. */
export type SyncStreams = Record<SyncGroup, MediaStream>;

/**
 * The streams of one link: the camera's is the local stream when there is one; any stream will do, since only
 * its id travels, but it must exist from the first offer on, as a section cannot change it later.
 */
export const syncStreams = (camera: MediaStream | null): SyncStreams => ({
    camera: camera ?? new MediaStream(),
    screen: new MediaStream(),
});

/**
 * Lets an answered section announce its group's stream; the offer's transceivers start with none. A browser
 * without `setStreams` sends without one, and the peer then plays that sound and picture unaligned.
 */
const announceStream = (sender: RTCRtpSender, stream: MediaStream): void => {
    if (typeof sender.setStreams !== 'function') return;
    try {
        sender.setStreams(stream);
    } catch (error) {
        console.warn('could not announce the stream of a section', error);
    }
};

/** The tracks a captured screen puts on the links; none while nothing is shared or the sound was left out. */
export const screenTracksOf = (screen: MediaStream | null): ScreenTracks => ({
    screen: screen?.getVideoTracks()[0] ?? null,
    screenAudio: screen?.getAudioTracks()[0] ?? null,
});

/**
 * The offerer's sections, in the order every link has them: audio, camera, screen, screen audio. A section
 * without a track is still created, so the layout never varies. The camera comes from `addTrack` when there is
 * a track; both audio sections are made directly, with their priority and RED. Every section announces its sync
 * group's stream from the start, so the peer plays the microphone with the camera and the screen's sound with
 * its picture, whenever their tracks arrive. The microphone and camera are sent only with a local stream.
 */
export const addOfferedSections = (
    connection: RTCPeerConnection,
    tracks: LocalTracks,
    stream: MediaStream | null,
    streams: SyncStreams,
): SectionSenders => {
    const of = (section: DirectSection) => [streams[SYNC_GROUP[section]]];
    const microphone = stream ? tracks.audio : null;
    const camera = stream ? tracks.video : null;
    const audio = addAudioSection(connection, microphone ?? 'audio', {
        direction: 'sendrecv',
        streams: of('audio'),
    }).sender;
    const video = camera
        ? connection.addTrack(camera, ...of('video'))
        : connection.addTransceiver('video', { direction: 'sendrecv', streams: of('video') }).sender;
    const screen = connection.addTransceiver(tracks.screen ?? 'video', {
        direction: 'sendrecv',
        streams: of('screen'),
    }).sender;
    const screenAudio = addAudioSection(connection, tracks.screenAudio ?? 'audio', {
        direction: 'sendrecv',
        streams: of('screenAudio'),
    }).sender;
    return { audio, video, screen, screenAudio };
};

/**
 * The answerer's sections, after the offer was applied: the first audio and first video transceiver carry the
 * microphone and the camera, the second ones the screen's sound and picture. All four send as well as receive,
 * both audio sections get RED and their priority as the offerer's do, and each announces its sync group's
 * stream in the answer.
 * The senders are stored in `senders` as they are attached. The local tracks may change while a
 * `replaceTrack` is awaited, so they are read again afterwards and any section that is behind is replaced
 * again, until every section carries the current track.
 */
export const attachAnsweredSections = async (
    connection: RTCPeerConnection,
    local: () => LocalTracks,
    senders: SectionSenders,
    streams: SyncStreams,
): Promise<void> => {
    const transceivers = connection.getTransceivers();
    const [audio, screenAudio] = transceivers.filter((t) => t.receiver.track.kind === 'audio');
    const [video, screen] = transceivers.filter((t) => t.receiver.track.kind === 'video');
    const found = { audio, video, screen, screenAudio };
    for (const section of [audio, screenAudio]) if (section) protectAnsweredAudio(section);
    for (const kind of DIRECT_SECTIONS) {
        const transceiver = found[kind];
        if (transceiver) announceStream(transceiver.sender, streams[SYNC_GROUP[kind]]);
    }
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
