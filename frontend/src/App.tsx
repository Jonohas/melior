import { useRef } from "react";
import "./index.css";

export function App() {
    const localVideoRef = useRef<HTMLVideoElement>(null);
    const remoteVideoRef = useRef<HTMLVideoElement>(null);

    const connectToStream = async (channelId: string) => {
        const pc = new RTCPeerConnection({
            iceServers: [{ urls: "stun:stun.l.google.com:19302" }]
        });

        // 1. Get local camera/mic
        const localStream = await navigator.mediaDevices.getUserMedia({
            video: true,
            audio: false // set true if you want audio
        });

        // 2. Display local video
        if (localVideoRef.current) {
            localVideoRef.current.srcObject = localStream;
        }

        // 3. Add tracks to peer connection
        for (const track of localStream.getTracks()) {
            pc.addTrack(track, localStream);
        }

        // 4. Handle incoming tracks (the echo from server)
        pc.ontrack = (event) => {
            console.log("Received remote track:", event.track.kind);
            if (remoteVideoRef.current && event.streams[0]) {
                remoteVideoRef.current.srcObject = event.streams[0];
            }
        };

        // 5. Create Offer
        const offer = await pc.createOffer();
        await pc.setLocalDescription(offer);

        // 6. Wait for ICE gathering to complete
        await new Promise<void>((resolve) => {
            if (pc.iceGatheringState === "complete") {
                resolve();
            } else {
                pc.onicegatheringstatechange = () => {
                    if (pc.iceGatheringState === "complete") {
                        resolve();
                    }
                };
            }
        });

        // 7. Send to server
        const response = await fetch(`/api/channels/${channelId}/connect`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                sdp: pc.localDescription?.sdp,
                displayName: "Test User",
                sdpType: "offer"
            })
        });

        const { response: answerSdp } = await response.json();
        console.log("Answer SDP:", answerSdp);

        // 8. Set Answer
        await pc.setRemoteDescription(new RTCSessionDescription({
            type: "answer",
            sdp: answerSdp
        }));

        console.log("WebRTC Connection Established");
    };

    return (
        <div className="flex justify-center items-center flex-col w-full h-screen gap-4">
            <div className="flex gap-4">
                <div>
                    <p>Local (your camera)</p>
                    <video
                        ref={localVideoRef}
                        autoPlay
                        muted
                        playsInline
                        className="w-[640px] h-[480px] bg-black"
                        style={{ transform: "scaleX(-1)" }}
                    />
                </div>
                <div>
                    <p>Remote (echo from server)</p>
                    <video
                        ref={remoteVideoRef}
                        autoPlay
                        playsInline
                        className="w-[640px] h-[480px] bg-black"
                    />
                </div>
            </div>
            <div className="card">
                <form
                    className="form w-full flex"
                    onSubmit={(event) => {
                        event.preventDefault();
                        connectToStream("8fc5eba8-28cb-470b-ab4b-7e554b09a09c");
                    }}
                >
                    <input type="submit" className="submit" value="Connect" />
                </form>
            </div>
        </div>
    );
}

export default App;