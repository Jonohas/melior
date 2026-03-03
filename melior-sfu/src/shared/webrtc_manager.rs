use crate::streaming::tracks::camera_track::CameraTrack;
use crate::streaming::tracks::track::Track;
use crate::structs::channel::Channel;
use crate::structs::client_registry::ClientRegistry;
use axum::extract::ws::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tokio::sync::Mutex;
use uuid::Uuid;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::{APIBuilder, API};
use webrtc::ice_transport::ice_candidate::RTCIceCandidate;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::policy::ice_transport_policy::RTCIceTransportPolicy;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtp::packet::Packet;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::TrackLocalWriter;
use webrtc::track::track_remote::TrackRemote;

pub struct WebRtcManager {
    api: Arc<API>,
    channels: HashMap<String, Arc<Mutex<Channel>>>,
    pub peers: HashMap<String, Arc<webrtc::peer_connection::RTCPeerConnection>>,
}

impl<'a> WebRtcManager {
    pub fn new() -> Self {
        let mut media_engine = MediaEngine::default();
        media_engine.register_default_codecs();
        let api = Arc::new(APIBuilder::new().with_media_engine(media_engine).build());

        let peers = HashMap::new();
        Self {
            api,
            channels: HashMap::new(),
            peers,
        }
    }

    pub fn get_config(&self) -> RTCConfiguration {
        RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["turn:127.0.0.1:3478?transport=udp".to_string()],
                username: "username".to_string(),
                credential: "password".to_string(),
            }],
            ..Default::default()
        }
    }

    pub async fn create_response_to_offer(
        &mut self,
        offer_sdp: String,
        session_id: String,
        channel_id: String,
        client_registry: Arc<ClientRegistry>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["turn:127.0.0.1:3478?transport=udp".to_string()],
                username: "username".to_string(),
                credential: "password".to_string(),
            }],
            ..Default::default()
        };
        let pc = Arc::new(self.api.new_peer_connection(config).await?);
        let orig_session_id = session_id.clone();

        pc.on_ice_connection_state_change(Box::new(move |state| {
            println!("ICE state: {:?}", state);
            Box::pin(async {})
        }));

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
                            "candidate": cand_json.candidate,
                            "sdp_mid": cand_json.sdp_mid,
                            "sdp_mline_index": cand_json.sdp_mline_index,
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

        if self.channels.get(&channel_id).is_none() {
            println!("Creating new channel {}", channel_id);
            self.channels.insert(
                channel_id.clone(),
                Arc::new(Mutex::new(Channel::new(session_id.clone()))),
            );
        }

        let channel = self.channels.get(&channel_id).unwrap().clone();
        let channel_id_copy = channel_id.clone();

        println!("here");

        pc.on_track(Box::new(move |track: Arc<TrackRemote>, _, _| {
            println!("Handling track on channel: {}", channel_id);

            let session_id = session_id.clone();
            let channel_id = channel_id.clone();
            let channel = channel.clone();
            Box::pin(async move {
                let camera_track = Arc::new(CameraTrack::new(track, session_id));
                let tx = camera_track.sender.clone();
                channel.lock().await.add_track(camera_track.clone());

                println!("Added track to channel {}", channel_id.clone());
                tokio::spawn(async move {
                    while let Ok((packet, _)) = camera_track.stream.read_rtp().await {
                        if let Err(e) = tx.send(packet) {
                            eprintln!("Error writing to track: {}", e);
                            break;
                        }
                    }
                });
            })
        }));

        println!("here 1");

        let channel = self.channels.get(&channel_id_copy).unwrap().clone();
        let channel_lock = channel.lock().await;

        // 3. Iterate and filter over the `tracks` HashMap
        // This example filters for tracks where the track ID starts with "audio"
        let filtered_tracks: Vec<(&String, &Sender<Packet>)> = channel_lock
            .tracks
            .iter()
            .filter(|(track_id, _sender)| {
                // Replace this with your actual filtering condition
                track_id.ne(&&orig_session_id)
            })
            .collect();

        println!("here 2");

        // Now you can do something with the filtered tracks
        for (track_id, sender) in filtered_tracks.clone() {
            let mut track = sender.subscribe();

            let created_response_track = Arc::new(TrackLocalStaticRTP::new(
                RTCRtpCodecCapability {
                    mime_type: "video/VP8".to_string(),
                    ..Default::default()
                },
                track_id.to_string(),
                Uuid::new_v4().to_string(),
            ));

            pc.add_track(created_response_track.clone()).await.unwrap();

            tokio::spawn(async move {
                while let Ok(packet) = track.recv().await {
                    if let Err(_) = created_response_track.write_rtp(&packet).await {
                        break; // Exit if the peer connection is closed
                    }
                }
            });
        }

        println!(
            "Connecting peer to {}, current connected peer count: {}",
            filtered_tracks.len(),
            channel_lock.tracks.len()
        );
        drop(channel_lock);

        pc.set_remote_description(RTCSessionDescription::offer(offer_sdp)?)
            .await?;
        let answer = pc.create_answer(None).await?;

        let mut gatherer = pc.gathering_complete_promise().await;
        pc.set_local_description(answer).await?;
        let _ = gatherer.recv().await;

        println!("here 3");

        self.peers.insert(orig_session_id.clone(), pc.clone());

        println!("here 4");
        Ok(pc.local_description().await.unwrap().sdp)
    }

    pub async fn connect(
        &self,
        offer_sdp: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let config = self.get_config();
        let pc = self.api.new_peer_connection(config).await?;
        pc.set_remote_description(RTCSessionDescription::offer(offer_sdp)?)
            .await?;
        let answer = pc.create_answer(None).await?;
        let mut gatherer = pc.gathering_complete_promise().await;
        pc.set_local_description(answer).await?;
        Ok(pc.local_description().await.unwrap().sdp)
    }
}
