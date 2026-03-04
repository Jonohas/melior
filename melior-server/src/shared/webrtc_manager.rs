use crate::structs::client_registry::ClientRegistry;
use axum::extract::ws::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::{APIBuilder, API};
use webrtc::ice_transport::ice_candidate::RTCIceCandidate;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::TrackLocalWriter;
use webrtc::track::track_remote::TrackRemote;
use crate::shared::track_registry::TrackRegistry;

pub struct WebRtcManager {
    api: Arc<API>,
    pub peers: Arc<Mutex<HashMap<String, Arc<webrtc::peer_connection::RTCPeerConnection>>>>,
}

impl WebRtcManager {
    pub fn new() -> Self {
        let mut media_engine = MediaEngine::default();
        media_engine.register_default_codecs();
        let api = Arc::new(APIBuilder::new().with_media_engine(media_engine).build());

        Self {
            api,
            peers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn create_response_to_offer(
        &mut self,
        offer_sdp: String,
        session_id: String,
        client_registry: Arc<ClientRegistry>,
        track_registry: TrackRegistry,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["turn:10.54.89.137:3478?transport=udp".to_string()],
                username: "username".to_string(),
                credential: "password".to_string(),
            }],
            ..Default::default()
        };

        let pc = Arc::new(self.api.new_peer_connection(config).await?);

        // ICE state logging
        pc.on_ice_connection_state_change(Box::new(move |state| {
            println!("ICE state: {:?}", state);
            Box::pin(async {})
        }));

        // ICE candidate forwarding
        let session_id_clone = session_id.clone();
        let client_registry_clone = client_registry.clone();

        pc.on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidate>| {
            let session_id = session_id_clone.clone();
            let client_registry = client_registry_clone.clone();

            Box::pin(async move {
                if let Some(cand) = candidate {
                    if let Ok(cand_json) = cand.to_json() {
                        let ice_msg = serde_json::json!({
                        "channel": "ice",
                        "payload": cand_json
                    });

                        if let Err(e) = client_registry
                            .send_to_session(
                                &session_id,
                                Message::text(serde_json::to_string(&ice_msg).unwrap()),
                            )
                            .await
                        {
                            eprintln!("Failed to send ICE candidate: {}", e);
                        }
                    }
                }
            })
        }));

        // Add all existing tracks from other peers before answering
        let existing_tracks = track_registry.get_other_tracks(&session_id).await;
        for track in &existing_tracks {
            pc.add_track(track.clone()).await?;
        }

        // on_track: create forwarder, store it, renegotiate other peers
        let session_id_for_track = session_id.clone();
        let track_registry_clone = track_registry.clone();
        let client_registry_for_track = client_registry.clone();
        let peers_ref = self.peers.clone();

        pc.on_track(Box::new(move |track: Arc<TrackRemote>, _, _| {
            let session_id = session_id_for_track.clone();
            let track_registry = track_registry_clone.clone();
            let client_registry = client_registry_for_track.clone();
            let peers = peers_ref.clone();

            Box::pin(async move {
                let codec = track.codec();

                let local_track = Arc::new(TrackLocalStaticRTP::new(
                    RTCRtpCodecCapability {
                        mime_type: codec.capability.mime_type.clone(),
                        ..Default::default()
                    },
                    Uuid::new_v4().to_string(),
                    Uuid::new_v4().to_string(),
                ));

                // Store in registry
                track_registry
                    .add_track(&session_id, local_track.clone())
                    .await;

                // Forward RTP packets
                let writer = local_track.clone();
                tokio::spawn(async move {
                    while let Ok((packet, _)) = track.read_rtp().await {
                        if writer.write_rtp(&packet).await.is_err() {
                            break;
                        }
                    }
                });

                // Renegotiate all OTHER peers to receive this new track
                let peers_locked = peers.lock().await;
                for (peer_session_id, peer_pc) in peers_locked.iter() {
                    if peer_session_id == &session_id {
                        continue;
                    }

                    if let Err(e) = peer_pc.add_track(local_track.clone()).await {
                        eprintln!("Failed to add track to peer {}: {}", peer_session_id, e);
                        continue;
                    }

                    let offer = match peer_pc.create_offer(None).await {
                        Ok(o) => o,
                        Err(e) => {
                            eprintln!("Failed to create offer for {}: {}", peer_session_id, e);
                            continue;
                        }
                    };

                    if let Err(e) = peer_pc.set_local_description(offer).await {
                        eprintln!("Failed to set local desc for {}: {}", peer_session_id, e);
                        continue;
                    }

                    if let Some(desc) = peer_pc.local_description().await {
                        let msg = serde_json::json!({
                        "channel": "sdp_offer",
                        "payload": {
                            "type": "offer",
                            "sdp": desc.sdp
                        }
                    });

                        if let Err(e) = client_registry
                            .send_to_session(
                                peer_session_id,
                                Message::text(serde_json::to_string(&msg).unwrap()),
                            )
                            .await
                        {
                            eprintln!("Failed to send renegotiation offer to {}: {}", peer_session_id, e);
                        }
                    }
                }

                println!(
                    "Track from {} stored and renegotiated to other peers",
                    session_id
                );
            })
        }));

        pc.set_remote_description(RTCSessionDescription::offer(offer_sdp)?)
            .await?;
        let answer = pc.create_answer(None).await?;
        pc.set_local_description(answer).await?;

        self.peers.lock().await.insert(session_id, pc.clone());

        Ok(pc.local_description().await.unwrap().sdp)
    }

    pub async fn handle_renegotiation_answer(
        &self,
        session_id: &str,
        answer_sdp: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let peers = self.peers.lock().await;
        let pc = peers.get(session_id).ok_or("Peer not found")?;
        pc.set_remote_description(RTCSessionDescription::answer(answer_sdp)?)
            .await?;
        Ok(())
    }

    pub async fn add_track_to_peer(
        &self,
        session_id: &str,
        track: Arc<TrackLocalStaticRTP>,
        client_registry: Arc<ClientRegistry>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let peers = self.peers.lock().await;
        let pc = peers.get(session_id).ok_or("Peer not found")?;

        pc.add_track(track).await?;

        let offer = pc.create_offer(None).await?;
        pc.set_local_description(offer).await?;

        let sdp = pc.local_description().await.unwrap();
        let msg = serde_json::json!({
        "channel": "sdp_offer",
        "payload": {
            "type": "offer",
            "sdp": sdp.sdp
        }
    });

        client_registry
            .send_to_session(
                session_id,
                Message::text(serde_json::to_string(&msg).unwrap()),
            )
            .await?;

        Ok(())
    }
}