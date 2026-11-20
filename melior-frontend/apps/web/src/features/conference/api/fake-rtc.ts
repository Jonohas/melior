import { vi } from 'vitest';

type FakeTrack = { id: string; kind: string; enabled?: boolean };

export class FakeSender {
    track: FakeTrack | null;
    replaced: Array<FakeTrack | null> = [];

    constructor(track: FakeTrack | null) {
        this.track = track;
    }

    async replaceTrack(track: FakeTrack | null) {
        this.replaced.push(track);
        this.track = track;
    }
}

export class FakeTransceiver {
    direction: string;
    mid: string | null = null;
    sender: FakeSender;
    receiver: { track: FakeTrack };

    constructor(kind: string, track: FakeTrack | null, direction: string, index: number) {
        this.direction = direction;
        this.sender = new FakeSender(track);
        this.receiver = { track: { id: `remote-${index}`, kind } };
    }
}

export class FakeDataChannel {
    sent: string[] = [];
    readyState = 'connecting';
    closed = false;
    onopen: (() => void) | null = null;
    onmessage: ((event: { data: unknown }) => void) | null = null;

    constructor(
        readonly label: string,
        readonly init: unknown,
    ) {}

    send(data: string) {
        this.sent.push(data);
    }

    close() {
        this.closed = true;
    }

    /** The channel opens, as the browser does once the transport is up. */
    open() {
        this.readyState = 'open';
        this.onopen?.();
    }

    receive(data: unknown) {
        this.onmessage?.({ data });
    }
}

/** What an offer carries when the test does not say otherwise: audio, camera, screen. */
const OFFERED_SECTIONS = ['audio', 'video', 'video'];

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
    ontrack: ((event: { track: FakeTrack; transceiver: FakeTransceiver }) => void) | null = null;
    transceivers: FakeTransceiver[] = [];
    dataChannels: FakeDataChannel[] = [];
    onconnectionstatechange: (() => void) | null = null;

    constructor() {
        FakePeerConnection.instances.push(this);
    }

    addTrack(track: FakeTrack) {
        this.tracksAdded += 1;
        return this.addTransceiver(track, { direction: 'sendrecv' }).sender;
    }

    addTransceiver(source: FakeTrack | string, init: { direction: string }) {
        const track = typeof source === 'string' ? null : source;
        const kind = typeof source === 'string' ? source : source.kind;
        const transceiver = new FakeTransceiver(kind, track, init.direction, this.transceivers.length);
        this.transceivers.push(transceiver);
        return transceiver;
    }

    createDataChannel(label: string, init: unknown) {
        const channel = new FakeDataChannel(label, init);
        this.dataChannels.push(channel);
        return channel;
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
        if (description.type === 'offer' && this.transceivers.length === 0) {
            // The offer's sections arrive as receive-only transceivers.
            for (const kind of OFFERED_SECTIONS) this.addTransceiver(kind, { direction: 'recvonly' });
        }
    }

    async addIceCandidate() {}

    async getStats() {
        return { forEach: (visit: (entry: Record<string, unknown>) => void) => this.statsEntries.forEach(visit) };
    }

    getSenders() {
        return this.transceivers.map((transceiver) => transceiver.sender);
    }

    getTransceivers() {
        return this.transceivers;
    }

    /** The peer's media on one section arrives, as the browser fires `ontrack`. */
    fireTrack(index: number) {
        const transceiver = this.transceivers[index];
        if (transceiver) this.ontrack?.({ track: transceiver.receiver.track, transceiver });
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

export class FakeMediaStream {
    tracks: FakeTrack[] = [];

    constructor(tracks: FakeTrack[] = []) {
        this.tracks = tracks;
    }

    addTrack(track: FakeTrack) {
        this.tracks.push(track);
    }

    getTracks() {
        return this.tracks;
    }

    getAudioTracks() {
        return this.tracks.filter((track) => track.kind === 'audio');
    }

    getVideoTracks() {
        return this.tracks.filter((track) => track.kind === 'video');
    }
}

export const stubRtc = () => {
    FakePeerConnection.instances = [];
    vi.stubGlobal('RTCPeerConnection', FakePeerConnection);
    vi.stubGlobal('MediaStream', FakeMediaStream);
};
