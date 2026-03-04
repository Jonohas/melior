import {useEffect, useState} from "react";
import "./index.css";
import {useWebsocket} from "@/hooks/useWebsocket.ts";
import {useUserStream} from "@/hooks/useUserStream.ts";
import {useWebRTC} from "@/hooks/useWebRTC.ts";
import {VideoPlayer} from "@/VideoPlayer.tsx";
import {RemoteVideo} from "@/remoteVideo.tsx";

export function App() {
    const [loading, setLoading] = useState(true);
    
    const {sessionId, channelId, websocketRef} = useWebsocket();
    const {stream, initializeStream, localVideoRef} = useUserStream();
    const {initializePeerConnection, remoteStreams} = useWebRTC();

    useEffect(() => {
        
        const initialize = async () => {
            if (websocketRef.current) {
                const stream = await initializeStream();
                await initializePeerConnection(websocketRef.current, sessionId, channelId, stream);
                setLoading(false);
            }
        }

        initialize();
    }, []);
    
    return (
        <div className="flex justify-center items-center flex-col w-full h-screen gap-4">
            {loading && <div>Loading...</div>}
            {!loading && (
                <>
                    <div>{channelId} - {sessionId}</div>
                    <div className="flex gap-4">
                        <VideoPlayer ref={localVideoRef} />
                    </div>
                    <div>
                        {remoteStreams.map((stream) => (
                            <RemoteVideo key={stream.id} stream={stream} />
                        ))}
                    </div>
                </>
            )}
        </div>
    );
}

export default App;
