import {useCallback, useRef, useState} from "react";


export const useUserStream = () => {
    const [stream, setStream] = useState<MediaStream | null>(null);
    const videoRef = useRef<HTMLVideoElement>(null);
    
    const initializeStream = useCallback(async () => {
        
        const stream = await navigator.mediaDevices.getUserMedia({
            video: true,
            audio: false
        });
        
        videoRef.current = videoRef.current || document.createElement("video");
        
        setStream(stream);
        
        return stream;
    }, [setStream]);


    const localVideoRef = useCallback(
        (node: HTMLVideoElement | null) => {
            if (node && stream) {
                node.srcObject = stream;
            }
        },
        [stream],
    );
    
    return {stream, initializeStream, localVideoRef}
};