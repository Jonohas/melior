use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use webrtc::api::{APIBuilder, API};
use webrtc::api::media_engine::MediaEngine;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::track::track_remote::TrackRemote;
use crate::streaming::tracks::camera_track::CameraTrack;
use crate::streaming::tracks::track::Track;
use crate::structs::channel::Channel;

pub struct WebRtcManager {
    api: Arc<API>,
    channels: HashMap<String, Arc<Mutex<Channel>>>,
}

impl<'a> WebRtcManager {
    pub fn new() -> Self {
        let mut media_engine = MediaEngine::default();
        media_engine.register_default_codecs();
        let api = Arc::new(APIBuilder::new()
            .with_media_engine(media_engine)
            .build());
        Self { api, channels: HashMap::new() }
    }

    pub async fn create_response_to_offer(
        &mut self,
        offer_sdp: String,
        session_id: String,
        channel_id: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let pc = self.api.new_peer_connection(RTCConfiguration::default()).await?;

        if self.channels.get(&channel_id).is_none() {
            println!("Creating new channel {}", channel_id);
            self.channels.insert(
                channel_id.clone(),
                Arc::new(Mutex::new(Channel::new(session_id.clone()))),
            );
        }

        let channel = self.channels.get(&channel_id).unwrap().clone();

        // add all tracks from channel to the new peer.

        pc.on_track(Box::new(move |track: Arc<TrackRemote>, _, _| {
            let session_id = session_id.clone();
            let channel = channel.clone();
            Box::pin(async move {
                let camera_track =
                    Arc::new(CameraTrack::new(track, session_id));
                let tx = camera_track.sender.clone();
                channel.lock().await.add_track(camera_track.clone());
                tokio::spawn(async move {
                    while let Ok((packet, _)) =
                        camera_track.stream.read_rtp().await
                    {
                        if let Err(e) = tx.send(packet) {
                            eprintln!("Error writing to track: {}", e);
                            break;
                        }
                    }
                });
            })
        }));

        pc.set_remote_description(RTCSessionDescription::offer(offer_sdp)?)
            .await?;
        let answer = pc.create_answer(None).await?;

        let mut gatherer = pc.gathering_complete_promise().await;
        pc.set_local_description(answer).await?;
        let _ = gatherer.recv().await;

        Ok(pc.local_description().await.unwrap().sdp)
    }
}