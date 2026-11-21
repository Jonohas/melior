use super::backend;
use super::codecs::{ReceiverAudio, outlet_codec};
use super::forwarder::{Forwarder, ForwarderRegistry, OutletTrack};
use super::offer::sections_not_sending;
use async_trait::async_trait;
use melior_application::{MediaError, MediaEvents, MediaOffer, MediaSession};
use melior_domain::{IceCandidate, MediaKind, Sdp, SessionId, Track, TrackId};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use tokio::sync::Mutex as AsyncMutex;
use tracing::{debug, warn};
use webrtc::ice_transport::ice_candidate::{RTCIceCandidate, RTCIceCandidateInit};
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtcp::payload_feedbacks::full_intra_request::FullIntraRequest;
use webrtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication;
use webrtc::rtp_transceiver::RTCRtpTransceiver;
use webrtc::rtp_transceiver::rtp_codec::RTPCodecType;
use webrtc::rtp_transceiver::rtp_sender::RTCRtpSender;
use webrtc::rtp_transceiver::rtp_transceiver_direction::RTCRtpTransceiverDirection;
use webrtc::track::track_local::TrackLocal;
use webrtc::track::track_remote::TrackRemote;

/// One media section of the connection that sends a track to the participant.
///
/// A media section can never be removed from a session description, so an outlet
/// is kept when its track ends and carries the next track of the same codec. A
/// connection therefore has as many sections as it ever received tracks at once,
/// however many participants come and go.
struct Outlet {
    kind: MediaKind,
    /// The codec the section was negotiated for; it only carries tracks of that codec.
    mime_type: String,
    track: Arc<OutletTrack>,
    transceiver: Arc<RTCRtpTransceiver>,
    /// The published track being sent, if any.
    carrying: Arc<Mutex<Option<Arc<Forwarder>>>>,
}

impl Outlet {
    fn can_carry(&self, forwarder: &Forwarder) -> bool {
        self.kind == forwarder.kind
            && self.mime_type.eq_ignore_ascii_case(&forwarder.codec.mime_type)
    }

    /// Stops sending and marks the media section as unused.
    async fn quiet(&self) {
        let carried = self.carrying.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(forwarder) = carried {
            forwarder.detach(&self.track);
        }
        self.transceiver.set_direction(RTCRtpTransceiverDirection::Inactive).await;
    }
}

#[derive(Default)]
struct Outlets {
    /// Outlets in use, by the track each carries.
    active: HashMap<TrackId, Outlet>,
    /// Outlets whose track ended, waiting for another one.
    idle: Vec<Outlet>,
}

/// A track the client sends, with the receiver section it arrives on.
struct Incoming {
    forwarder: Arc<Forwarder>,
    transceiver: Arc<RTCRtpTransceiver>,
}

type IncomingTracks = Arc<Mutex<Vec<Incoming>>>;

/// [`MediaSession`] over one webrtc-rs peer connection.
pub(super) struct WebRtcSession {
    id: SessionId,
    connection: Arc<RTCPeerConnection>,
    forwarders: ForwarderRegistry,
    outlets: AsyncMutex<Outlets>,
    /// What the client sends, until it ends or the client stops sending it.
    incoming: IncomingTracks,
    /// Subscriptions added since the last answer; they need a keyframe to start.
    awaiting_keyframe: Mutex<Vec<Arc<Forwarder>>>,
}

impl WebRtcSession {
    pub fn new(
        id: SessionId,
        connection: Arc<RTCPeerConnection>,
        forwarders: ForwarderRegistry,
        events: Weak<dyn MediaEvents>,
    ) -> Self {
        Self::report_ice_candidates(&id, &connection, events.clone());
        Self::report_connection_state(&id, &connection, events.clone());
        let incoming = IncomingTracks::default();
        Self::forward_published_tracks(&id, &connection, &forwarders, &incoming, events);

        Self {
            incoming,
            id,
            connection,
            forwarders,
            outlets: AsyncMutex::new(Outlets::default()),
            awaiting_keyframe: Mutex::new(Vec::new()),
        }
    }

    fn report_ice_candidates(
        id: &SessionId,
        connection: &RTCPeerConnection,
        events: Weak<dyn MediaEvents>,
    ) {
        let id = id.clone();
        connection.on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidate>| {
            if let (Some(candidate), Some(events)) = (candidate, events.upgrade()) {
                match candidate.to_json() {
                    Ok(init) => events.ice_candidate(&id, from_rtc_candidate(init)),
                    Err(error) => warn!(session = %id, %error, "unserializable ICE candidate"),
                }
            }
            Box::pin(async {})
        }));
    }

    fn report_connection_state(
        id: &SessionId,
        connection: &RTCPeerConnection,
        events: Weak<dyn MediaEvents>,
    ) {
        let id = id.clone();
        connection.on_peer_connection_state_change(Box::new(move |state| {
            debug!(session = %id, %state, "peer connection state");
            if state == RTCPeerConnectionState::Connected
                && let Some(events) = events.upgrade()
            {
                // Off the callback: the application will renegotiate on this very
                // connection, which must not happen inside its own event handler.
                let id = id.clone();
                tokio::spawn(async move { events.connected(&id).await });
            }
            Box::pin(async {})
        }));
    }

    /// Every track the client sends gets a forwarder; the application decides who
    /// subscribes to it.
    fn forward_published_tracks(
        id: &SessionId,
        connection: &Arc<RTCPeerConnection>,
        forwarders: &ForwarderRegistry,
        incoming: &IncomingTracks,
        events: Weak<dyn MediaEvents>,
    ) {
        let id = id.clone();
        let incoming = incoming.clone();
        let publisher = Arc::downgrade(connection);
        let forwarders = forwarders.clone();

        connection.on_track(Box::new(move |remote: Arc<TrackRemote>, _, transceiver| {
            let kind = match remote.kind() {
                RTPCodecType::Audio => MediaKind::Audio,
                RTPCodecType::Video => MediaKind::Video,
                RTPCodecType::Unspecified => return Box::pin(async {}),
            };
            let forwarder = Arc::new(Forwarder::new(&remote, kind, publisher.clone()));
            forwarders.insert(forwarder.clone());
            lock(&incoming).push(Incoming { forwarder: forwarder.clone(), transceiver });

            let id = id.clone();
            let incoming = incoming.clone();
            let forwarders = forwarders.clone();
            let events = events.clone();
            tokio::spawn(async move {
                // Announce, forward until the track ends, then retract: in this
                // order, so the application never sees "ended" before "published".
                if let Some(events) = events.upgrade() {
                    let track = Track::new(forwarder.id.clone(), forwarder.kind);
                    events.track_published(&id, track).await;
                }
                forwarder.run(remote).await;
                lock(&incoming).retain(|track| !Arc::ptr_eq(&track.forwarder, &forwarder));
                forwarders.remove(&forwarder.id);
                if let Some(events) = events.upgrade() {
                    events.track_ended(&id, &forwarder.id).await;
                }
            });
            Box::pin(async {})
        }));
    }

    /// Ends the tracks that arrive on the given sections: the client no longer sends
    /// there, so nothing would ever make their reads fail. Ends them as a failed
    /// read does; the task that runs the forwarder retracts the track.
    fn end_tracks_in(&self, mids: &HashSet<String>) {
        if mids.is_empty() {
            return;
        }
        let incoming = lock(&self.incoming);
        for track in incoming.iter() {
            let silent = track.transceiver.mid().is_some_and(|mid| mids.contains(mid.as_str()));
            if silent {
                track.forwarder.stop();
            }
        }
    }

    /// The local description as it should be sent to the client.
    async fn local_sdp(&self, fallback: RTCSessionDescription) -> Sdp {
        let description = self.connection.local_description().await.unwrap_or(fallback);
        Sdp::new(description.sdp)
    }

    /// The audio codecs this participant's connection negotiated. Its client
    /// offers first, so the description is there by the time anything is
    /// forwarded to it; without one, no RED.
    async fn receiver_audio(&self) -> ReceiverAudio {
        self.connection
            .remote_description()
            .await
            .map(|description| ReceiverAudio::from_sdp(&description.sdp))
            .unwrap_or_default()
    }

    /// Relays the participant's keyframe requests (PLI/FIR) for an outlet to the
    /// publisher of whatever track the outlet carries at that moment. Reading the
    /// sender's RTCP is also what drives its interceptors (NACK, reports).
    fn relay_keyframe_requests(
        sender: Arc<RTCRtpSender>,
        carrying: Arc<Mutex<Option<Arc<Forwarder>>>>,
    ) {
        tokio::spawn(async move {
            while let Ok((packets, _)) = sender.read_rtcp().await {
                let wants_keyframe = packets.iter().any(|packet| {
                    let packet = packet.as_any();
                    packet.is::<PictureLossIndication>() || packet.is::<FullIntraRequest>()
                });
                if !wants_keyframe {
                    continue;
                }
                let forwarder = carrying.lock().unwrap_or_else(PoisonError::into_inner).clone();
                if let Some(forwarder) = forwarder {
                    forwarder.request_keyframe().await;
                }
            }
        });
    }

    /// Opens a new media section sending `forwarder`'s track.
    async fn open_outlet(&self, forwarder: &Arc<Forwarder>) -> Result<Outlet, MediaError> {
        let receiver = self.receiver_audio().await;
        let (codec, repack) = outlet_codec(&forwarder.codec, &receiver);
        let track = Arc::new(OutletTrack::new(codec, repack));
        let local: Arc<dyn TrackLocal + Send + Sync> = track.local.clone();
        let sender = self.connection.add_track(local).await.map_err(backend)?;

        let mut owner = None;
        for transceiver in self.connection.get_transceivers().await {
            if Arc::ptr_eq(&transceiver.sender().await, &sender) {
                owner = Some(transceiver);
                break;
            }
        }
        let transceiver =
            owner.ok_or_else(|| MediaError::Backend("new sender has no transceiver".to_owned()))?;

        let carrying = Arc::new(Mutex::new(Some(forwarder.clone())));
        Self::relay_keyframe_requests(sender, carrying.clone());
        forwarder.attach(track.clone());
        Ok(Outlet {
            kind: forwarder.kind,
            mime_type: forwarder.codec.mime_type.clone(),
            track,
            transceiver,
            carrying,
        })
    }

    /// Puts `forwarder`'s track on an idle outlet.
    async fn reuse_outlet(outlet: &Outlet, forwarder: &Arc<Forwarder>) {
        outlet.track.switch_source();
        *outlet.carrying.lock().unwrap_or_else(PoisonError::into_inner) = Some(forwarder.clone());
        forwarder.attach(outlet.track.clone());
        outlet.transceiver.set_direction(RTCRtpTransceiverDirection::Sendonly).await;
    }
}

#[async_trait]
impl MediaSession for WebRtcSession {
    async fn apply_offer(&self, offer: Sdp) -> Result<Sdp, MediaError> {
        let offer = RTCSessionDescription::offer(offer.into_string()).map_err(backend)?;
        let silent = sections_not_sending(&offer.sdp);
        self.connection.set_remote_description(offer).await.map_err(backend)?;
        self.end_tracks_in(&silent);
        let answer = self.connection.create_answer(None).await.map_err(backend)?;
        self.connection.set_local_description(answer.clone()).await.map_err(backend)?;
        Ok(self.local_sdp(answer).await)
    }

    async fn create_offer(&self) -> Result<MediaOffer, MediaError> {
        // Under the outlet lock, so the list of what each section carries matches
        // the description.
        let outlets = self.outlets.lock().await;
        let offer = self.connection.create_offer(None).await.map_err(backend)?;
        self.connection.set_local_description(offer.clone()).await.map_err(backend)?;
        // Every section has its mid once the offer is the local description.
        let carrying = outlets
            .active
            .iter()
            .filter_map(|(track, outlet)| {
                Some((outlet.transceiver.mid()?.to_string(), track.clone()))
            })
            .collect();
        drop(outlets);
        Ok(MediaOffer { sdp: self.local_sdp(offer).await, carrying })
    }

    async fn apply_answer(&self, answer: Sdp) -> Result<(), MediaError> {
        let answer = RTCSessionDescription::answer(answer.into_string()).map_err(backend)?;
        self.connection.set_remote_description(answer).await.map_err(backend)?;

        let started: Vec<Arc<Forwarder>> = std::mem::take(
            &mut *self.awaiting_keyframe.lock().unwrap_or_else(PoisonError::into_inner),
        );
        for forwarder in started {
            forwarder.request_keyframe().await;
        }
        Ok(())
    }

    async fn add_ice_candidate(&self, candidate: IceCandidate) -> Result<(), MediaError> {
        self.connection.add_ice_candidate(to_rtc_candidate(candidate)).await.map_err(backend)?;
        Ok(())
    }

    async fn subscribe(&self, track: &TrackId) -> Result<(), MediaError> {
        let forwarder =
            self.forwarders.get(track).ok_or_else(|| MediaError::TrackNotFound(track.clone()))?;

        let mut outlets = self.outlets.lock().await;
        if outlets.active.contains_key(track) {
            return Ok(());
        }
        let outlet = match outlets.idle.iter().position(|outlet| outlet.can_carry(&forwarder)) {
            Some(index) => {
                let outlet = outlets.idle.swap_remove(index);
                Self::reuse_outlet(&outlet, &forwarder).await;
                outlet
            }
            None => self.open_outlet(&forwarder).await?,
        };
        outlets.active.insert(track.clone(), outlet);
        drop(outlets);

        self.awaiting_keyframe.lock().unwrap_or_else(PoisonError::into_inner).push(forwarder);
        Ok(())
    }

    async fn unsubscribe(&self, track: &TrackId) -> Result<(), MediaError> {
        self.awaiting_keyframe
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|forwarder| &forwarder.id != track);

        let mut outlets = self.outlets.lock().await;
        if let Some(outlet) = outlets.active.remove(track) {
            outlet.quiet().await;
            outlets.idle.push(outlet);
        }
        Ok(())
    }

    async fn close(&self) {
        // Closing ends the read loops of this session's forwarders, which then
        // deregister themselves.
        if let Err(error) = self.connection.close().await {
            debug!(session = %self.id, %error, "closing peer connection");
        }
        // Publishers must stop writing to this connection's outlets.
        let outlets = std::mem::take(&mut *self.outlets.lock().await);
        for outlet in outlets.active.into_values() {
            let carried = outlet.carrying.lock().unwrap_or_else(PoisonError::into_inner).take();
            if let Some(forwarder) = carried {
                forwarder.detach(&outlet.track);
            }
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn to_rtc_candidate(candidate: IceCandidate) -> RTCIceCandidateInit {
    RTCIceCandidateInit {
        candidate: candidate.candidate,
        sdp_mid: candidate.sdp_mid,
        sdp_mline_index: candidate.sdp_mline_index,
        username_fragment: candidate.username_fragment,
    }
}

fn from_rtc_candidate(candidate: RTCIceCandidateInit) -> IceCandidate {
    IceCandidate {
        candidate: candidate.candidate,
        sdp_mid: candidate.sdp_mid,
        sdp_mline_index: candidate.sdp_mline_index,
        username_fragment: candidate.username_fragment,
    }
}
