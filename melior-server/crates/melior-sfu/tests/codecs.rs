//! The SFU negotiates VP8, Opus and RED over Opus only, whatever else a client offers.

mod common;

use common::{TestServer, new_peer_connection, next_json};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::tungstenite::Message;
use webrtc::rtp_transceiver::rtp_codec::RTPCodecType;

#[tokio::test]
async fn the_answer_keeps_only_vp8_opus_and_red() {
    let server = TestServer::start().await;
    server.voice_channel("codecs").await;
    let cookie = server.login("alice", &["member"]).await;
    let request = server.ws_request(Some("codecs"), Some(&cookie));
    let (socket, _) = tokio_tungstenite::connect_async(request).await.expect("connect");
    let (mut ws_tx, mut ws_rx) = socket.split();
    assert_eq!(next_json(&mut ws_rx).await["channel"], "session");

    // Every default codec, as a browser offers them: VP8, VP9, H264, Opus, G722, ...
    let connection = new_peer_connection().await;
    connection.add_transceiver_from_kind(RTPCodecType::Audio, None).await.unwrap();
    connection.add_transceiver_from_kind(RTPCodecType::Video, None).await.unwrap();
    let offer = connection.create_offer(None).await.unwrap();
    assert!(offer.sdp.contains("VP9/90000") && offer.sdp.contains("H264/90000"));
    let message =
        json!({ "channel": "sdp_offer", "payload": { "type": "offer", "sdp": offer.sdp } });
    ws_tx.send(Message::text(message.to_string())).await.unwrap();

    let answer = loop {
        let message = next_json(&mut ws_rx).await;
        if message["channel"] == "sdp_answer" {
            break message["payload"]["sdp"].as_str().unwrap().to_owned();
        }
    };
    let rtpmaps: Vec<&str> = answer.lines().filter(|line| line.starts_with("a=rtpmap:")).collect();
    assert!(rtpmaps.iter().any(|line| line.contains("VP8/90000")), "{answer}");
    assert!(rtpmaps.iter().any(|line| line.contains("opus/48000")), "{answer}");
    for other in ["VP9", "H264", "AV1", "G722", "PCMU", "PCMA"] {
        assert!(!rtpmaps.iter().any(|line| line.contains(other)), "{other} in {answer}");
    }
}
