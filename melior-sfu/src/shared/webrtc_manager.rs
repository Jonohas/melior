use std::collections::HashMap;
use std::sync::Arc;
use webrtc::api::{APIBuilder, API};
use webrtc::api::media_engine::MediaEngine;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::track::track_local::{TrackLocal, TrackLocalWriter};
use webrtc::track::track_remote::TrackRemote;
use webrtc::rtp_transceiver::rtp_receiver::RTCRtpReceiver;
use crate::streaming::tracks::camera_track::CameraTrack;
use crate::streaming::tracks::track::Track;
use crate::structs::channel::Channel;

pub struct WebRtcManager {
    api: Arc<API>,
    channels: HashMap<String, Channel>
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

        // How many tracks is the client trying to send? Camera and audio? Or just audio?
        let camera_track = CameraTrack::new(&pc, session_id.clone());

        if self.channels.get(&channel_id).is_none() {
            println!("Creating new channel {}", channel_id);
            self.channels.insert(channel_id.clone(), Channel::new(session_id.clone()));
        }

        let channel = self.channels.get(&channel_id).ok_or_else(|| panic!("Channel not found for ID: {}", channel_id)).unwrap();

        for track in channel.tracks.iter() {
            if (track.0).eq(&session_id) { continue; }
            pc.add_track(track.1.clone()).await?;
        }

        camera_track.register_track().await?;

        let video_track_clone = camera_track.get_track();

        // a Box is needed here because the closure size is unknown at compile time, so the runtime can allocate on the heap instead. So boxes exist on the heap
        pc.on_track(Box::new(move |track: Arc<TrackRemote>, _: Arc<RTCRtpReceiver>, _| {
            // here we handle what happens to a track that is incoming.
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