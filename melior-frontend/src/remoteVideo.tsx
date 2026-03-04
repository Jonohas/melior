import {useEffect, useRef} from "react";

export const RemoteVideo = ({ stream }: { stream: MediaStream }) => {
    const ref = useRef<HTMLVideoElement>(null);

    useEffect(() => {
        if (ref.current) {
            ref.current.srcObject = stream;
        }
    }, [stream]);

    return <div className={"border-2 rounded border-green-500"}><video ref={ref} autoPlay playsInline /></div>;
};