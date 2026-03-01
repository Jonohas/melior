import {useCallback, useEffect, useRef, useState} from "react";
import "./index.css";
import {VideoPlayer} from "@/features/streaming/components/VideoPlayer.tsx";
import {useStreamStore} from "@/features/streaming/store/streamingStore.ts";

export function App() {
    const {localStream, initialize, peerConnection, setStream, disconnect, streams} = useStreamStore();
    const [loading, setLoading] = useState(true);
    const [messages, setMessages] = useState<string[]>([]);
    const [input, setInput] = useState('');
    const [ws, setWs] = useState<WebSocket | null>(null);

    useEffect(() => {
        const websocket = new WebSocket('ws://localhost:5000');
        setWs(websocket);

        websocket.onopen = () => console.log('Connected to WebSocket server');
        websocket.onmessage = (event) => {
            setMessages((prevMessages) => [...prevMessages, event.data]);
        };
        websocket.onclose = () => console.log('Disconnected from WebSocket server');

        // Cleanup on unmount
        return () => websocket.close();
    }, []);

    const sendMessage = () => {
        if (ws && ws.readyState === WebSocket.OPEN) {
            ws.send(input);
            setInput('');
        }
    };
    
    useEffect(() => {
        const init = async () => {
            await initialize();
            setLoading(false);
        }
        
        init();
    }, [initialize, setLoading])

    const localVideoRef = useCallback(
        (node: HTMLVideoElement | null) => {
            if (node && localStream) {
                node.srcObject = localStream;
            }
        },
        [localStream],
    );
    
    const connectToStream = useCallback(async (channelId: string) => {
        const stream = await navigator.mediaDevices.getUserMedia({
            video: true,
            audio: false
        });

        setStream(stream);

        // 2. Find the transceiver we created during initialization
        const videoTrack = stream.getVideoTracks()[0];
        const sender = peerConnection.getSenders().find(s => s.track === null || s.track?.kind === "video");

        if (sender && videoTrack) {
            // This attaches the camera to the pre-gathered ICE session
            await sender.replaceTrack(videoTrack);
        }
        
        console.log(peerConnection.localDescription?.sdp);
        
        const response = await fetch(`/api/channels/${channelId}/connect`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                sdp: peerConnection.localDescription?.sdp,
                displayName: "Test User",
                sdpType: "offer"
            })
        });

        const { response: answerSdp } = await response.json();

        // 8. Set Answer
        await peerConnection.setRemoteDescription(new RTCSessionDescription({
            type: "answer",
            sdp: answerSdp
        }));

    }, [peerConnection, setStream])

    return (
        <div className="flex justify-center items-center flex-col w-full h-screen gap-4">
            {loading && <div>Loading...</div>}
            {!loading && (
                <>
                    <div className="flex gap-4">
                        <VideoPlayer ref={localVideoRef} />
                        {streams?.map((stream, i) => (
                            <video
                                key={stream.id}
                                autoPlay
                                playsInline
                                ref={(el) => {
                                    if (el) el.srcObject = stream;
                                }}
                                className="w-[400px] h-[255px] bg-black"
                            />
                        ))}
                    </div>
                    <div className="bottom-overview">
                        <button className="submit" onClick={(event) => {
                            event.preventDefault();
                            connectToStream("33846b5c-e01b-41fe-9042-54a84dd96993");
                        }}>Connect</button>
                        <button className="submit" onClick={disconnect}>Disconnect</button>
                    </div>

                    <div className="notification-center">
                        <h2>Real-Time Notifications</h2>
                        <div className="messages">
                            {messages.map((message, index) => (
                                <p key={index}>{message}</p>
                            ))}
                        </div>
                        <input
                            type="text"
                            value={input}
                            onChange={(e) => setInput(e.target.value)}
                            placeholder="Type a message"
                        />
                        <button onClick={sendMessage}>Send</button>
                    </div>
                </>
            )}
        </div>
    );
}

export default App;