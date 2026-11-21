import { preferredVideoCodecs } from '@/features/conference/domain';

/**
 * Restricts every video section of `connection` to VP8, before an offer or an answer is made. A browser that
 * cannot set codec preferences keeps negotiating its own list; the server then accepts only VP8 from it.
 */
export const preferVideoCodec = (connection: RTCPeerConnection): void => {
    const capabilities = typeof RTCRtpReceiver === 'undefined' ? null : RTCRtpReceiver.getCapabilities?.('video');
    if (!capabilities) return;
    const codecs = preferredVideoCodecs(capabilities.codecs);
    for (const transceiver of connection.getTransceivers()) {
        if (transceiver.receiver.track.kind !== 'video') continue;
        if (typeof transceiver.setCodecPreferences !== 'function') continue;
        try {
            transceiver.setCodecPreferences(codecs);
        } catch (error) {
            console.warn('could not prefer VP8 on a section', error);
        }
    }
};
