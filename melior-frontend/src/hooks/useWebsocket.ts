import {useEffect, useRef, useState} from "react";
import {v4 as uuidv4} from 'uuid';


export const useWebsocket = () => {
    const [sessionId, setSessionId] = useState<string>(uuidv4());
    const [channelId, setChannelId] = useState<string>("general");
    const websocketRef = useRef<WebSocket | null>(null);
    
    useEffect(() => {
        websocketRef.current = new WebSocket(`ws://127.0.0.1:5000/ws?session_id=${sessionId}&channel_id=${channelId}`);
        
        websocketRef.current.onopen = () => console.log('Connected to WebSocket server');
        websocketRef.current.onclose = () => console.log('Disconnected from WebSocket server');
        
        return () => {
            if (websocketRef.current) {
                websocketRef.current.close();
            }
        };
    }, [])
    
    return {sessionId, channelId, websocketRef}
};