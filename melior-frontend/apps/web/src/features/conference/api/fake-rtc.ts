import { vi } from 'vitest';

type FakeTrack = { id: string; kind: string; enabled?: boolean };

export class FakeSender {
    track: FakeTrack | null;
    replaced: Array<FakeTrack | null> = [];

    constructor(track: FakeTrack | null) {
        this.track = track;
    }

    /** Makes the next `replaceTrack` calls fail, as a sender that cannot take the track does. */
    failing = false;
    /** Runs while a `replaceTrack` is in flight, before it settles. */
    duringReplace: (() => void) | null = null;

    /** The encodings as `setParameters` last left them. */
    encodings: Record<string, unknown>[] = [{ active: true }];

    getParameters() {
        return { encodings: this.encodings.map((encoding) => ({ ...encoding })) };
    }

    async setParameters(parameters: { encodings: Record<string, unknown>[] }) {
        this.encodings = parameters.encodings;
    }

    async replaceTrack(track: FakeTrack | null) {
        this.duringReplace?.();
        if (this.failing) throw new Error('replaceTrack failed');
        this.replaced.push(track);
        this.track = track;
    }
}

export class FakeTransceiver {
    direction: string;
    mid: string | null = null;
    /** Whether a track was ever attached to the sender; addTrack only reuses a section that never sent. */
    hasSent: boolean;
    sender: FakeSender;
    receiver: { track: FakeTrack };
    /** The codecs `setCodecPreferences` was given, in order. */
    codecPreferences: Array<{ mimeType: string }> | null = null;
    /** The `sendEncodings` the section was created with. */
    sendEncodings: unknown[] | undefined;

    constructor(kind: string, track: FakeTrack | null, direction: string, index: number, sendEncodings?: unknown[]) {
        this.direction = direction;
        this.sendEncodings = sendEncodings;
        this.hasSent = track !== null;
        this.sender = new FakeSender(track);
        this.receiver = { track: { id: `remote-${index}`, kind } };
    }

    setCodecPreferences(codecs: Array<{ mimeType: string }>) {
        this.codecPreferences = codecs;
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
const OFFERED_SECTIONS = ['audio', 'video', 'video', 'audio'];

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
    /** Runs after `setRemoteDescription` applied an offer, once the transceivers exist. */
    afterSetRemoteOffer: (() => void) | null = null;
    ontrack: ((event: { track: FakeTrack; transceiver: FakeTransceiver }) => void) | null = null;
    transceivers: FakeTransceiver[] = [];
    dataChannels: FakeDataChannel[] = [];
    onconnectionstatechange: (() => void) | null = null;

    constructor() {
        FakePeerConnection.instances.push(this);
    }

    /** Like Chromium: reuses a section that never sent and matches the kind, else adds one. */
    addTrack(track: FakeTrack) {
        this.tracksAdded += 1;
        const idle = this.transceivers.find((t) => t.receiver.track.kind === track.kind && !t.hasSent);
        if (!idle) return this.addTransceiver(track, { direction: 'sendrecv' }).sender;
        idle.sender.track = track;
        idle.hasSent = true;
        idle.direction = 'sendrecv';
        return idle.sender;
    }

    /** Like `removeTrack`: the sender stops sending, the section stays and only receives. */
    removeTrack(sender: FakeSender) {
        sender.track = null;
        const section = this.transceivers.find((t) => t.sender === sender);
        if (section) section.direction = 'recvonly';
    }

    addTransceiver(source: FakeTrack | string, init: { direction: string; sendEncodings?: unknown[] }) {
        const track = typeof source === 'string' ? null : source;
        const kind = typeof source === 'string' ? source : source.kind;
        const index = this.transceivers.length;
        const transceiver = new FakeTransceiver(kind, track, init.direction, index, init.sendEncodings);
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
            this.afterSetRemoteOffer?.();
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

/** The audio codecs the fake browser can receive, in Chromium's order. */
export const FAKE_AUDIO_CODECS = [
    { mimeType: 'audio/opus', clockRate: 48000, channels: 2 },
    { mimeType: 'audio/red', clockRate: 48000, channels: 2 },
    { mimeType: 'audio/PCMU', clockRate: 8000 },
];

export const stubRtc = () => {
    FakePeerConnection.instances = [];
    vi.stubGlobal('RTCPeerConnection', FakePeerConnection);
    vi.stubGlobal('RTCRtpReceiver', {
        getCapabilities: (kind: string) => (kind === 'audio' ? { codecs: FAKE_AUDIO_CODECS } : null),
    });
    vi.stubGlobal('MediaStream', FakeMediaStream);
};
