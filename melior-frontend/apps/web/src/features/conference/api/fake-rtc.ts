import { vi } from 'vitest';

/** Test double for the parts of RTCPeerConnection the conference code uses. Only imported by tests. */
export class FakePeerConnection {
    static instances: FakePeerConnection[] = [];

    connectionState = 'new';
    remoteDescription: { type: string; sdp: string } | null = null;
    localDescription: { type: string; sdp: string } | null = null;
    closed = false;
    tracksAdded = 0;
    offersCreated = 0;
    statsEntries: Record<string, unknown>[] = [];
    onicecandidate: ((event: { candidate: { toJSON: () => object } | null }) => void) | null = null;
    /** Runs inside `setLocalDescription`, before it resolves: where the browser fires its first candidates. */
    duringSetLocalDescription: (() => void) | null = null;
    ontrack: unknown = null;
    onconnectionstatechange: (() => void) | null = null;

    constructor() {
        FakePeerConnection.instances.push(this);
    }

    addTrack() {
        this.tracksAdded += 1;
    }

    async createOffer() {
        this.offersCreated += 1;
        return { type: 'offer', sdp: 'offer-sdp' };
    }

    async createAnswer() {
        return { type: 'answer', sdp: 'answer-sdp' };
    }

    async setLocalDescription(description: { type: string; sdp: string }) {
        this.localDescription = description;
        this.duringSetLocalDescription?.();
    }

    async setRemoteDescription(description: { type: string; sdp: string }) {
        this.remoteDescription = description;
    }

    async addIceCandidate() {}

    async getStats() {
        return { forEach: (visit: (entry: Record<string, unknown>) => void) => this.statsEntries.forEach(visit) };
    }

    getSenders() {
        return [];
    }

    getTransceivers() {
        return [];
    }

    close() {
        this.closed = true;
    }

    /** Fires a local ICE candidate, as the browser does. */
    fireCandidate(candidate: string) {
        this.onicecandidate?.({ candidate: { toJSON: () => ({ candidate }) } });
    }

    /** Moves the connection to a state and fires the handler, as the browser does. */
    setState(state: string) {
        this.connectionState = state;
        this.onconnectionstatechange?.();
    }
}

class FakeMediaStream {
    tracks: unknown[] = [];

    addTrack(track: unknown) {
        this.tracks.push(track);
    }
}

export const stubRtc = () => {
    FakePeerConnection.instances = [];
    vi.stubGlobal('RTCPeerConnection', FakePeerConnection);
    vi.stubGlobal('MediaStream', FakeMediaStream);
};
