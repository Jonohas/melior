import {type Ref} from "react";


type VideoPlayerProps = {
    ref?: Ref<HTMLVideoElement>
};

export const VideoPlayer = ({ref}: VideoPlayerProps) => {
    return (
        <div>
            <p>Local (your camera)</p>
            <video
                ref={ref}
                autoPlay
                muted
                playsInline
                className="w-[400px] h-[255px] bg-black"
                style={{ transform: "scaleX(-1)" }}
            />
        </div>
    );
}