/**
 * Pauses or resumes sending on a connection without renegotiating: `encodings[*].active` on every sender.
 * A paused sender keeps its track published; it only stops sending packets.
 */
export const setSendersActive = async (connection: RTCPeerConnection, active: boolean): Promise<void> => {
    for (const sender of connection.getSenders()) {
        if (!sender.track) continue;
        const parameters = sender.getParameters();
        if (!parameters.encodings?.length) continue;
        for (const encoding of parameters.encodings) encoding.active = active;
        try {
            await sender.setParameters(parameters);
        } catch (error) {
            console.warn('setParameters failed', error);
        }
    }
};
