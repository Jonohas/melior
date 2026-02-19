use std::sync::Arc;
use webrtc::api::{APIBuilder, API};
use webrtc::api::media_engine::MediaEngine;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::TrackLocalWriter;
use webrtc::track::track_remote::TrackRemote;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::rtp_transceiver::rtp_receiver::RTCRtpReceiver;

pub struct Track {

}

pub struct WebRtcManager {
    api: API,
}

impl WebRtcManager {
    pub fn new() -> Self {
        let mut media_engine = MediaEngine::default();
        media_engine.register_default_codecs();
        let api = APIBuilder::new()
            .with_media_engine(media_engine)
            .build();
        Self { api }
    }

    pub async fn create_response_to_offer(
        &self,
        offer_sdp: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let pc = Arc::new(self.api.new_peer_connection(RTCConfiguration::default()).await?);

        // Create outgoing video track
        let video_track = Arc::new(TrackLocalStaticRTP::new(
            RTCRtpCodecCapability {
                mime_type: "video/VP8".to_string(),
                ..Default::default()
            },
            "video".to_string(),
            "echo-stream".to_string(),
        ));

        pc.add_track(video_track.clone()).await?;

        let video_track_clone = video_track.clone();
        pc.on_track(Box::new(move |track: Arc<TrackRemote>, _: Arc<RTCRtpReceiver>, _| {
            let outgoing = video_track_clone.clone();

            Box::pin(async move {
                println!("Received track: kind={}, codec={}",
                         track.kind(),
                         track.codec().capability.mime_type
                );

                tokio::spawn(async move {
                    let mut buf = vec![0u8; 1500];
                    while let Ok((rtp_packet, _)) = track.read(&mut buf).await {
                        if let Err(e) = outgoing.write_rtp(&rtp_packet).await {
                            eprintln!("Error writing to track: {}", e);
                            break;
                        }
                    }
                });
            })
        }));

        pc.set_remote_description(RTCSessionDescription::offer(offer_sdp)?).await?;
        let answer = pc.create_answer(None).await?;

        let mut gatherer = pc.gathering_complete_promise().await;
        pc.set_local_description(answer).await?;
        let _ = gatherer.recv().await;

        Ok(pc.local_description().await.unwrap().sdp)
    }
}