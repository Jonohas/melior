import {useCallback, useEffect, useRef, useState} from "react";


export const useWebRTC = () => {
    const [peerConnection, setPeerConnection] = useState<RTCPeerConnection | null>(null);

    const [remoteStreams, setRemoteStreams] = useState<MediaStream[]>([]);

    const initializePeerConnection = useCallback(async (ws: WebSocket, session_id: string, channel_id: string, stream: MediaStream) => {
        const pc = new RTCPeerConnection({
            iceServers: [
                {
                    urls: "turn:127.0.0.1:3478?transport=udp",
                    username: "username",
                    credential: "password",
                },
            ],
        });

        stream.getTracks().forEach((track) => {
            pc.addTrack(track, stream);
        });

        const candidateQueue: RTCIceCandidateInit[] = [];

        pc.ontrack = (event) => {
            console.log("ontrack", event.track.kind, event.track.readyState, event.streams.length);
        };
        
        pc.onicecandidate = (event) => {
            if (event.candidate) {
                ws.send(JSON.stringify({
                    channel: "ice",
                    session_id,
                    channel_id,
                    payload: {
                        candidate: event.candidate.candidate,
                        sdp_mid: event.candidate.sdpMid,
                        sdp_mline_index: event.candidate.sdpMLineIndex,
                    }
                }));
            }
        }

        ws.onmessage = async (message) => {
            const data = JSON.parse(message.data);

            if (data.channel === "ice") {
                const candidate = new RTCIceCandidate(data.payload);
                // 2. Check if we are ready to consume candidates
                if (pc.remoteDescription && pc.remoteDescription.type) {
                    await pc.addIceCandidate(candidate);
                } else {
                    candidateQueue.push(data.payload);
                }
            }

            if (data.channel === "sdp_answer") {
                await pc.setRemoteDescription(new RTCSessionDescription(data.payload));
                // 3. Process the queue now that RemoteDescription is set
                while (candidateQueue.length) {
                    const cand = candidateQueue.shift();
                    if (cand) await pc.addIceCandidate(new RTCIceCandidate(cand));
                }
            }

            if (data.channel === "sdp_offer") {
                await pc.setRemoteDescription(
                    new RTCSessionDescription(data.payload)
                );
                while (candidateQueue.length) {
                    const cand = candidateQueue.shift();
                    if (cand) await pc.addIceCandidate(new RTCIceCandidate(cand));
                }
                const answer = await pc.createAnswer();
                await pc.setLocalDescription(answer);
                ws.send(
                    JSON.stringify({
                        channel: "sdp_answer",
                        session_id,
                        channel_id,
                        payload: answer,
                    })
                );
            }
        };

        const offer = await pc.createOffer();
        await pc.setLocalDescription(offer);

        ws.send(JSON.stringify({ channel: "sdp", session_id, channel_id, payload: offer}));
        
        setPeerConnection(pc);
        
        return pc;
    }, [setPeerConnection]);
    
    return {peerConnection, initializePeerConnection, remoteStreams};
};