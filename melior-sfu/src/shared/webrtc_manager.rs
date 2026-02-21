use std::collections::HashMap;
use std::sync::{Arc};
use tokio::sync::RwLock;
use webrtc::api::{APIBuilder, API};
use webrtc::api::media_engine::MediaEngine;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::track::track_local::TrackLocalWriter;
use webrtc::track::track_remote::TrackRemote;
use webrtc::rtp_transceiver::rtp_receiver::RTCRtpReceiver;
use crate::streaming::peer::peer::Peer;

pub struct WebRtcManager {
    api: Arc<API>,
    peers: RwLock<HashMap<String, Arc<Peer>>>,
}

impl WebRtcManager {
    pub fn new() -> Self {
        let mut media_engine = MediaEngine::default();
        media_engine.register_default_codecs();
        let api = Arc::new(APIBuilder::new()
            .with_media_engine(media_engine)
            .build());
        Self { api, peers: RwLock::new(HashMap::new()) }
    }

    pub async fn create_response_to_offer(
        &self,
        offer_sdp: String,
        session_id: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // before we write, we should look up the existing peers.
        
        let peer = Arc::new(Peer::new(self.api.clone()).await?);
        self.peers.write().await.insert(session_id, peer.clone());
        println!("Peers count: {}", self.peers.read().await.len());
        let pc = peer.clone().get_peer_connection();
        let video_track_clone = peer.get_track().unwrap().clone();

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