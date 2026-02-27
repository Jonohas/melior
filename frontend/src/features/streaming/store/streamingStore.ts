import {create} from "zustand";

type StreamStore = {
    localStream?: MediaStream
    peerConnection: RTCPeerConnection
    
    setStream: (stream: MediaStream) => void
    initialize: () => Promise<void>
    disconnect: () => void
    initialized: boolean
    streams?: readonly MediaStream[]
}

export const useStreamStore = create<StreamStore>()((set, get) => ({
    initialized: false,
    peerConnection: null as unknown as RTCPeerConnection,

    initialize: async () => {
        if (get().initialized) return;
        set({ initialized: true });

        const pc = new RTCPeerConnection({
            iceServers: [
                {
                    urls: "turn:127.0.0.1:3478?transport=udp",
                    username: "username",
                    credential: "password",
                },
            ],
        });

        pc.ontrack = (event) => {
            set({ streams: event.streams });
            console.log("Received remote track", event.streams);
        };

        pc.addTransceiver("video", { direction: "sendrecv" });
        const offer = await pc.createOffer();
        await pc.setLocalDescription(offer);

        await new Promise<void>((resolve) => {
            if (pc.iceGatheringState === "complete") return resolve();
            pc.onicegatheringstatechange = () => {
                if (pc.iceGatheringState === "complete") resolve();
            };
        });

        console.log(pc.localDescription?.sdp);
        console.log("ICE Gathering Finished - Ready to connect");

        set({ peerConnection: pc });
    },
    setStream: (stream) => set({ localStream: stream }),
    disconnect: () => get().peerConnection?.close(),
}));