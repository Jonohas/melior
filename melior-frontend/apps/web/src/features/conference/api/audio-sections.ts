import { redFirst } from '@/features/conference/domain';

/**
 * Speech goes before pictures when the link is short of room: the browser gives audio the larger share and,
 * where the network honours it, marks its packets for earlier delivery.
 */
const AUDIO_PRIORITY = { priority: 'high', networkPriority: 'high' } as const satisfies RTCRtpEncodingParameters;

/**
 * Lets the section send RED (each packet repeats the frame before it) when this browser has it. Must run before
 * the offer that carries the section; a browser without RED or without codec preferences keeps plain Opus.
 */
export const preferRedAudio = (transceiver: RTCRtpTransceiver): void => {
    if (typeof RTCRtpReceiver === 'undefined' || typeof transceiver.setCodecPreferences !== 'function') return;
    const codecs = redFirst(RTCRtpReceiver.getCapabilities?.('audio')?.codecs ?? []);
    if (!codecs) return;
    try {
        transceiver.setCodecPreferences(codecs);
    } catch (error) {
        console.warn('could not prefer redundant audio', error);
    }
};

/** How a section of our own is made; `RTCRtpTransceiverInit` with both fields required. */
export type SectionInit = { direction: RTCRtpTransceiverDirection; streams: MediaStream[] };

/** Adds an audio section this side offers: high priority, and RED where the browser has it. */
export const addAudioSection = (
    connection: RTCPeerConnection,
    source: MediaStreamTrack | 'audio',
    init: SectionInit,
): RTCRtpTransceiver => {
    const transceiver = connection.addTransceiver(source, { ...init, sendEncodings: [{ ...AUDIO_PRIORITY }] });
    preferRedAudio(transceiver);
    return transceiver;
};

/**
 * Gives an audio sender that the other side's offer created the same priority as `addAudioSection` does. A
 * sender whose encodings do not exist yet is left as it is.
 */
const raiseAudioPriority = async (sender: RTCRtpSender): Promise<void> => {
    try {
        const parameters = sender.getParameters();
        if (!parameters.encodings?.length) return;
        for (const encoding of parameters.encodings) Object.assign(encoding, AUDIO_PRIORITY);
        await sender.setParameters(parameters);
    } catch (error) {
        console.warn('could not raise the audio priority', error);
    }
};

/**
 * Gives an audio section that the other side's offer created what `addAudioSection` gives our own: RED for the
 * answer, which must run before it, and the priority. Settles on its own; a failure only logs.
 */
export const protectAnsweredAudio = (transceiver: RTCRtpTransceiver): void => {
    preferRedAudio(transceiver);
    void raiseAudioPriority(transceiver.sender);
};
