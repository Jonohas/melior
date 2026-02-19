use webrtc::api::APIBuilder;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

pub struct WebRtcManager {

}

impl WebRtcManager {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn create_response_to_offer(&self, offer_sdp: String) -> Result<String, Box<dyn std::error::Error>> {
        let api = APIBuilder::new().build();
        let pc = api.new_peer_connection(RTCConfiguration::default()).await?;

        // In a streaming server, you'd usually add tracks or data channels here
        // pc.on_data_channel(...).await;

        pc.set_remote_description(RTCSessionDescription::offer(offer_sdp)?).await?;
        let answer = pc.create_answer(None).await?;

        // Gathering ICE candidates (Vanilla ICE simplified)
        let mut gatherer = pc.gathering_complete_promise().await;
        pc.set_local_description(answer).await?;
        let _ = gatherer.recv().await;

        Ok(pc.local_description().await.unwrap().sdp)
    }
}