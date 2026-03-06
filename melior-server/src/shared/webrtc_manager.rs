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
use webrtc::peer_connection::RTCPeerConnection;
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
        let pc = Arc::new(self.api.new_peer_connection(Self::ice_config()).await?);

        Self::setup_ice_logging(&pc);
        Self::setup_ice_candidate_forwarding(
            &pc,
            session_id.clone(),
            client_registry.clone(),
        );
        Self::setup_on_track(
            &pc,
            session_id.clone(),
            track_registry.clone(),
            client_registry.clone(),
            self.peers.clone(),
        );

        let existing_tracks = track_registry.get_other_tracks(&session_id).await;
        for track in &existing_tracks {
            pc.add_track(track.clone()).await?;
        }

        let sdp = Self::negotiate(&pc, offer_sdp).await?;

        self.peers.lock().await.insert(session_id, pc);

        Ok(sdp)
    }

    fn ice_config() -> RTCConfiguration {
        RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["turn:127.0.0.1:3478?transport=udp".to_string()],
                username: "username".to_string(),
                credential: "password".to_string(),
            }],
            ..Default::default()
        }
    }

    fn setup_ice_logging(pc: &Arc<RTCPeerConnection>) {
        pc.on_ice_connection_state_change(Box::new(move |state| {
            println!("ICE state: {:?}", state);
            Box::pin(async {})
        }));
    }

    fn setup_ice_candidate_forwarding(
        pc: &Arc<RTCPeerConnection>,
        session_id: String,
        client_registry: Arc<ClientRegistry>,
    ) {
        pc.on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidate>| {
            let session_id = session_id.clone();
            let client_registry = client_registry.clone();

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
                                Message::text(
                                    serde_json::to_string(&ice_msg).unwrap(),
                                ),
                            )
                            .await
                        {
                            eprintln!("Failed to send ICE candidate: {}", e);
                        }
                    }
                }
            })
        }));
    }

    fn setup_on_track(
        pc: &Arc<RTCPeerConnection>,
        session_id: String,
        track_registry: TrackRegistry,
        client_registry: Arc<ClientRegistry>,
        peers: Arc<Mutex<HashMap<String, Arc<RTCPeerConnection>>>>,
    ) {
        pc.on_track(Box::new(move |track: Arc<TrackRemote>, _, _| {
            let session_id = session_id.clone();
            let track_registry = track_registry.clone();
            let client_registry = client_registry.clone();
            let peers = peers.clone();

            Box::pin(async move {
                let local_track = Self::create_local_track(&track);

                track_registry
                    .add_track(&session_id, local_track.clone())
                    .await;

                Self::spawn_rtp_forwarder(track, local_track.clone());

                Self::renegotiate_peers(
                    &session_id,
                    local_track,
                    &peers,
                    &client_registry,
                )
                    .await;

                println!(
                    "Track from {} stored and renegotiated to other peers",
                    session_id
                );
            })
        }));
    }

    fn create_local_track(
        remote: &Arc<TrackRemote>,
    ) -> Arc<TrackLocalStaticRTP> {
        let codec = remote.codec();
        Arc::new(TrackLocalStaticRTP::new(
            RTCRtpCodecCapability {
                mime_type: codec.capability.mime_type.clone(),
                ..Default::default()
            },
            Uuid::new_v4().to_string(),
            Uuid::new_v4().to_string(),
        ))
    }

    fn spawn_rtp_forwarder(
        remote: Arc<TrackRemote>,
        local: Arc<TrackLocalStaticRTP>,
    ) {
        tokio::spawn(async move {
            while let Ok((packet, _)) = remote.read_rtp().await {
                if local.write_rtp(&packet).await.is_err() {
                    break;
                }
            }
        });
    }

    async fn renegotiate_peers(
        origin_session_id: &str,
        local_track: Arc<TrackLocalStaticRTP>,
        peers: &Arc<Mutex<HashMap<String, Arc<RTCPeerConnection>>>>,
        client_registry: &Arc<ClientRegistry>,
    ) {
        let peers_locked = peers.lock().await;
        for (peer_session_id, peer_pc) in peers_locked.iter() {
            if peer_session_id == origin_session_id {
                continue;
            }

            if let Err(e) =
                Self::renegotiate_single_peer(
                    peer_session_id,
                    peer_pc,
                    local_track.clone(),
                    client_registry,
                )
                    .await
            {
                eprintln!(
                    "Failed to renegotiate peer {}: {}",
                    peer_session_id, e
                );
            }
        }
    }

    async fn renegotiate_single_peer(
        peer_session_id: &str,
        peer_pc: &Arc<RTCPeerConnection>,
        local_track: Arc<TrackLocalStaticRTP>,
        client_registry: &Arc<ClientRegistry>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        peer_pc.add_track(local_track).await?;

        let offer = peer_pc.create_offer(None).await?;
        peer_pc.set_local_description(offer).await?;

        if let Some(desc) = peer_pc.local_description().await {
            let msg = serde_json::json!({
                "channel": "sdp_offer",
                "payload": {
                    "type": "offer",
                    "sdp": desc.sdp
                }
            });

            client_registry
                .send_to_session(
                    peer_session_id,
                    Message::text(serde_json::to_string(&msg).unwrap()),
                )
                .await?;
        }

        Ok(())
    }

    async fn negotiate(
        pc: &Arc<RTCPeerConnection>,
        offer_sdp: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        pc.set_remote_description(RTCSessionDescription::offer(offer_sdp)?)
            .await?;
        let answer = pc.create_answer(None).await?;
        pc.set_local_description(answer).await?;
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