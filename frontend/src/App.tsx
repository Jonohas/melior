import { useRef } from "react";
import "./index.css";

export function App() {
    const localVideoRef = useRef<HTMLVideoElement>(null);
    // const remoteVideoRef = useRef<HTMLVideoElement>(null);

    const connectToStream = async (channelId: string) => {
        console.log("Connecting to stream...");
        const pc = new RTCPeerConnection({
            iceServers: [
                {
                    urls: "turn:127.0.0.1:5349",
                    username: "username",
                    credential: "password",
                },
            ],
            iceTransportPolicy: "relay" // or "relay" to force TURN
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
            console.group("Received remote track");
            console.log(event.streams);
            console.groupEnd();
        };

        // 5. Create Offer
        const offer = await pc.createOffer();
        await pc.setLocalDescription(offer);
        
        console.log("Ice gathering...");

        await new Promise<void>((resolve) => {
            const timeout = setTimeout(() => {
                console.log("ICE Gathering timed out, proceeding with what we have.");
                resolve();
            }, 1000); // 1 second max wait

            pc.onicecandidate = (event) => {
                if (!event.candidate) {
                    console.log("Gathering finished naturally.");
                    clearTimeout(timeout);
                    resolve();
                } else if (event.candidate.type === "relay") {
                    console.log("Relay candidate found, proceeding immediately!");
                    clearTimeout(timeout);
                    resolve();
                }
            };
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
            </div>
            <div className="">
                <form
                    className="form w-full flex"
                    onSubmit={(event) => {
                        event.preventDefault();
                        connectToStream("33846b5c-e01b-41fe-9042-54a84dd96993");
                    }}
                >
                    <input type="submit" className="submit" value="Connect" />
                </form>
            </div>
        </div>
    );
}

export default App;