import "./index.css";

export function App() {
    const connectToStream = async (channelId: string) => {
        const pc = new RTCPeerConnection({
            iceServers: [{ urls: "stun:stun.l.google.com:19302" }]
        });

        // 1. Create DataChannel or add tracks before the offer
        pc.createDataChannel("test");

        // 2. Create Offer
        const offer = await pc.createOffer();
        await pc.setLocalDescription(offer);

        // 4. Send to .NET
        const response = await fetch(`/api/channels/${channelId}/connect`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ sdp: pc.localDescription?.sdp, displayName: "Test User", sdpType: "offer" })
        });

        const { response: answerSdp } = await response.json();

        console.log("Answer SDP:", answerSdp);

        // 5. Set Answer
        await pc.setRemoteDescription(new RTCSessionDescription({
            type: "answer",
            sdp: answerSdp
        }));

        console.log("WebRTC Connection Established");
    };

    
  return (
    <div className="flex justify-center items-center flex-col w-full h-screen">
        <div className="card">
            <form className={"form w-full flex"} onSubmit={(event) => {
                event.preventDefault();
                connectToStream('33846b5c-e01b-41fe-9042-54a84dd96993')
            }}>
                <input type={"submit"}  className={"submit"} value={"Add"}></input>
            </form>
        </div>
    </div>
  );
}

export default App;
