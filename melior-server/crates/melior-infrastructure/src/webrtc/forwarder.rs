use super::clock::{PublisherClock, StreamClock};
use super::codecs::Repack;
use super::layers::{LayerChoice, Step, vp8_keyframe_start};
use melior_domain::{MediaKind, ReceiverLayer, SimulcastLayer, StreamLabel, TrackId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock, Weak};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::Notify;
use tracing::debug;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtcp::packet::Packet as RtcpPacket;
use webrtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication;
use webrtc::rtcp::sender_report::SenderReport;
use webrtc::rtp::extension::abs_send_time_extension::unix2ntp;
use webrtc::rtp::packet::Packet;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::rtp_transceiver::rtp_receiver::RTCRtpReceiver;
use webrtc::track::track_local::TrackLocalWriter;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_remote::TrackRemote;

/// Publishers are asked for a keyframe at most this often per track.
const KEYFRAME_INTERVAL: Duration = Duration::from_millis(500);
/// A simulcast layer counts as arriving while its last packet is at most this old.
const LAYER_SILENCE: Duration = Duration::from_millis(1000);

/// The forwarders of every track currently published, across all sessions.
#[derive(Clone, Default)]
pub(super) struct ForwarderRegistry {
    inner: Arc<Mutex<HashMap<TrackId, Arc<Forwarder>>>>,
}

impl ForwarderRegistry {
    pub fn insert(&self, forwarder: Arc<Forwarder>) {
        self.lock().insert(forwarder.id.clone(), forwarder);
    }

    pub fn get(&self, id: &TrackId) -> Option<Arc<Forwarder>> {
        self.lock().get(id).cloned()
    }

    pub fn remove(&self, id: &TrackId) {
        self.lock().remove(id);
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<TrackId, Arc<Forwarder>>> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// One stream of a published track as it arrives: the track itself, or one of its
/// simulcast layers.
struct Source {
    layer: Option<SimulcastLayer>,
    ssrc: u32,
    /// When its last packet arrived, in milliseconds since the forwarder started.
    last_packet_ms: AtomicU64,
    /// What its publisher's latest sender report said, on the SFU's clock.
    clock: Mutex<Option<StreamClock>>,
    /// Which publisher's clock the stream runs on.
    publisher: Weak<PublisherClock>,
}

impl Source {
    fn clock(&self) -> Option<StreamClock> {
        *self.clock.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Relays one published track: RTP read from the publisher is handed to the
/// outlet of every participant who receives it. A camera sent in simulcast arrives
/// as several layers; each outlet carries one of them at a time.
pub(super) struct Forwarder {
    pub id: TrackId,
    pub kind: MediaKind,
    /// What the publisher sends; an outlet has to be opened with the same codec.
    pub codec: RTCRtpCodecCapability,
    publisher: Weak<RTCPeerConnection>,
    /// Puts the publisher's sender reports on the SFU's clock; shared by all its tracks.
    publisher_clock: Arc<PublisherClock>,
    started: Instant,
    sources: RwLock<Vec<Arc<Source>>>,
    /// Replaced as a whole on every change, so the packet loop only clones an `Arc`.
    outlets: RwLock<Arc<Vec<Arc<OutletTrack>>>>,
    last_keyframe_request: Mutex<Option<Instant>>,
    /// Set, and every [`Self::run`] woken, to end the track when its publisher
    /// stopped sending.
    stopped: AtomicBool,
    stop_signal: Notify,
    /// How many [`Self::run`] loops are reading, one per source.
    running: AtomicUsize,
}

impl Forwarder {
    pub fn new(
        remote: &TrackRemote,
        kind: MediaKind,
        publisher: Weak<RTCPeerConnection>,
        publisher_clock: Arc<PublisherClock>,
    ) -> Self {
        let forwarder = Self {
            id: TrackId::generate(),
            kind,
            codec: remote.codec().capability,
            publisher,
            publisher_clock,
            started: Instant::now(),
            sources: RwLock::new(Vec::new()),
            outlets: RwLock::new(Arc::new(Vec::new())),
            last_keyframe_request: Mutex::new(None),
            stopped: AtomicBool::new(false),
            stop_signal: Notify::new(),
            running: AtomicUsize::new(0),
        };
        forwarder.add_layer(remote);
        forwarder
    }

    /// Adds a stream that arrives for this track: another simulcast layer.
    pub fn add_layer(&self, remote: &TrackRemote) {
        let source = Source {
            layer: SimulcastLayer::from_rid(remote.rid()),
            ssrc: remote.ssrc(),
            last_packet_ms: AtomicU64::new(0),
            clock: Mutex::new(None),
            publisher: Arc::downgrade(&self.publisher_clock),
        };
        self.sources.write().unwrap_or_else(PoisonError::into_inner).push(Arc::new(source));
    }

    /// Starts sending this track to an outlet.
    pub fn attach(&self, outlet: Arc<OutletTrack>) {
        outlet.restart_layers();
        let mut outlets = self.outlets.write().unwrap_or_else(PoisonError::into_inner);
        let mut next: Vec<Arc<OutletTrack>> = outlets.as_ref().clone();
        next.push(outlet);
        *outlets = Arc::new(next);
    }

    /// Stops sending this track to an outlet.
    pub fn detach(&self, outlet: &Arc<OutletTrack>) {
        let mut outlets = self.outlets.write().unwrap_or_else(PoisonError::into_inner);
        let next: Vec<Arc<OutletTrack>> =
            outlets.iter().filter(|other| !Arc::ptr_eq(other, outlet)).cloned().collect();
        *outlets = Arc::new(next);
    }

    /// Ends every [`Self::run`], as a failing read does. Remembered if a loop is not
    /// waiting yet, so it cannot be missed.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.stop_signal.notify_waiters();
    }

    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// The simulcast layers whose packets still arrive.
    fn arriving_layers(&self) -> Vec<SimulcastLayer> {
        let now = self.elapsed_ms();
        let silence = u64::try_from(LAYER_SILENCE.as_millis()).unwrap_or(u64::MAX);
        self.sources
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|source| {
                let last = source.last_packet_ms.load(Ordering::Relaxed);
                last > 0 && now.saturating_sub(last) <= silence
            })
            .filter_map(|source| source.layer)
            .collect()
    }

    /// Takes a sender report of one of the track's streams: from now on outlets
    /// report that stream's clock to their receivers.
    fn sender_report(&self, report: &SenderReport) {
        let Some(source) = self.source_of(report.ssrc) else {
            return;
        };
        let local = self.publisher_clock.to_local(report.ntp_time, unix2ntp(SystemTime::now()));
        let clock = StreamClock::new(local, report.rtp_time, self.codec.clock_rate);
        *source.clock.lock().unwrap_or_else(PoisonError::into_inner) = Some(clock);
    }

    /// Reads the publisher's RTCP for one stream of the track until it ends or the
    /// track is stopped, keeping the clock its sender reports give. `rid` is empty
    /// for a track without simulcast.
    pub async fn read_reports(&self, receiver: Arc<RTCRtpReceiver>, rid: String) {
        loop {
            let stop = self.stop_signal.notified();
            tokio::pin!(stop);
            stop.as_mut().enable();
            if self.stopped.load(Ordering::SeqCst) {
                return;
            }
            let read = tokio::select! {
                read = async {
                    if rid.is_empty() {
                        receiver.read_rtcp().await
                    } else {
                        receiver.read_simulcast_rtcp(&rid).await
                    }
                } => read,
                () = &mut stop => return,
            };
            let Ok((packets, _)) = read else { return };
            for packet in &packets {
                if let Some(report) = packet.as_any().downcast_ref::<SenderReport>() {
                    self.sender_report(report);
                }
            }
        }
    }

    fn source_of(&self, ssrc: u32) -> Option<Arc<Source>> {
        let sources = self.sources.read().unwrap_or_else(PoisonError::into_inner);
        sources.iter().find(|source| source.ssrc == ssrc).cloned()
    }

    /// Reads one stream of the track until it ends or the track is stopped, handing
    /// each packet to the outlets that carry its layer. Returns whether this was the
    /// last stream of the track still being read: then the track has ended.
    pub async fn run(&self, remote: Arc<TrackRemote>) -> bool {
        self.running.fetch_add(1, Ordering::SeqCst);
        if let Some(source) = self.source_of(remote.ssrc()) {
            self.forward(&remote, &source).await;
        }
        // The report readers of a track that ended have nothing left to read.
        if self.running.load(Ordering::SeqCst) == 1 {
            self.stop();
        }
        self.running.fetch_sub(1, Ordering::SeqCst) == 1
    }

    async fn forward(&self, remote: &TrackRemote, source: &Arc<Source>) {
        loop {
            let stop = self.stop_signal.notified();
            tokio::pin!(stop);
            stop.as_mut().enable();
            if self.stopped.load(Ordering::SeqCst) {
                return;
            }
            let read = tokio::select! {
                read = remote.read_rtp() => read,
                () = &mut stop => return,
            };
            let Ok((mut packet, _)) = read else { return };
            // Header extension ids are negotiated per connection; the publisher's
            // (notably its MID and RID) would be wrong for subscribers. Each outlet's
            // track re-adds the ones its own connection negotiated.
            packet.header.extension = false;
            packet.header.extension_profile = 0;
            packet.header.extensions.clear();
            packet.header.extensions_padding = 0;
            source.last_packet_ms.store(self.elapsed_ms().max(1), Ordering::Relaxed);
            if self.hand_out(&packet, source).await {
                self.request_keyframe().await;
            }
        }
    }

    /// Writes `packet` to every outlet that carries its layer. Returns whether some
    /// outlet waits for a keyframe of another layer.
    async fn hand_out(&self, packet: &Packet, source: &Arc<Source>) -> bool {
        let layer = source.layer;
        let keyframe = layer.is_some() && vp8_keyframe_start(&packet.payload);
        let arriving = if layer.is_some() { self.arriving_layers() } else { Vec::new() };
        let outlets = self.outlets.read().unwrap_or_else(PoisonError::into_inner).clone();
        let mut waiting = false;
        for outlet in outlets.iter() {
            let (step, waits) = outlet.route(layer, keyframe, &arriving);
            waiting |= waits;
            if step != Step::Skip {
                // A failing subscriber must not stop the track for everyone else.
                outlet.write(packet, source).await;
            }
        }
        waiting
    }

    /// Asks the publisher for a keyframe on every stream of the track, so a new or
    /// recovering subscriber, or one moving to another layer, can start decoding.
    /// Rate limited; a no-op for audio.
    pub async fn request_keyframe(&self) {
        if self.kind != MediaKind::Video {
            return;
        }
        {
            let mut last =
                self.last_keyframe_request.lock().unwrap_or_else(PoisonError::into_inner);
            let now = Instant::now();
            if last.is_some_and(|at| now.duration_since(at) < KEYFRAME_INTERVAL) {
                return;
            }
            *last = Some(now);
        }
        let Some(publisher) = self.publisher.upgrade() else {
            return;
        };
        let requests: Vec<Box<dyn RtcpPacket + Send + Sync>> = self
            .sources
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|source| {
                Box::new(PictureLossIndication { sender_ssrc: 0, media_ssrc: source.ssrc })
                    as Box<dyn RtcpPacket + Send + Sync>
            })
            .collect();
        if let Err(error) = publisher.write_rtcp(&requests).await {
            debug!(track = %self.id, %error, "keyframe request failed");
        }
    }
}

/// The track one media section of a subscriber's connection sends from.
///
/// It belongs to the section for its whole life and carries one published track
/// after another. A receiver identifies a stream by the section's SSRC and expects
/// its sequence numbers and timestamps to keep rising, so when the source changes,
/// the new source's numbers are shifted to continue where the old ones stopped.
/// Without that, a receiver discards the new packets as late ones roughly every
/// other time.
pub(super) struct OutletTrack {
    pub local: Arc<TrackLocalStaticRTP>,
    continuity: Mutex<Continuity>,
    /// How packets change for this participant's codecs; fixed for the section's life.
    repack: Repack,
    layer: Mutex<OutletLayer>,
    /// The stream the outlet sends from and the timestamp offset it applies.
    timeline: Mutex<Timeline>,
    /// What it sent, for its sender reports. Wrapping, as RTCP counts are.
    packets: AtomicU32,
    octets: AtomicU32,
}

/// The stream an outlet last sent from, and how its timestamps are moved. Kept when
/// the outlet goes idle: the next stream continues this timeline.
#[derive(Default)]
struct Timeline {
    source: Option<Arc<Source>>,
    offset: u32,
}

impl Timeline {
    /// The outlet's own clock: its stream's, moved by the offset.
    fn clock(&self) -> Option<StreamClock> {
        Some(self.source.as_ref()?.clock()?.shifted(self.offset))
    }
}

/// The simulcast layer an outlet carries, and what its receiver's network allows.
#[derive(Debug, Clone, Copy, Default)]
struct OutletLayer {
    current: Option<SimulcastLayer>,
    receiver: ReceiverLayer,
}

impl OutletTrack {
    /// An outlet sending as `codec`, which `repack` turns published packets into,
    /// under the stream id `label`. A browser plays the sections that share a label
    /// in step; which tracks may share one is the domain's choice
    /// ([`melior_domain::choose_section`]). Who publishes a track is told next to
    /// each offer, never by the label.
    pub fn new(codec: RTCRtpCodecCapability, repack: Repack, label: &StreamLabel) -> Self {
        let local = TrackLocalStaticRTP::new(
            codec,
            TrackId::generate().as_str().to_owned(),
            label.as_str().to_owned(),
        );
        Self {
            local: Arc::new(local),
            continuity: Mutex::new(Continuity::default()),
            repack,
            layer: Mutex::new(OutletLayer::default()),
            timeline: Mutex::new(Timeline::default()),
            packets: AtomicU32::new(0),
            octets: AtomicU32::new(0),
        }
    }

    /// The sender report for this outlet's stream `ssrc` at wall-clock `ntp`: the
    /// publisher's clock moved by the outlet's timestamp offset. `None` until the
    /// publisher reported its clock.
    pub fn sender_report(&self, ssrc: u32, ntp: u64) -> Option<SenderReport> {
        let clock = self.timeline.lock().unwrap_or_else(PoisonError::into_inner).clock()?;
        Some(SenderReport {
            ssrc,
            ntp_time: ntp,
            rtp_time: clock.rtp_at(ntp),
            packet_count: self.packets.load(Ordering::Relaxed),
            octet_count: self.octets.load(Ordering::Relaxed),
            ..SenderReport::default()
        })
    }

    /// Forgets the layer carried: the outlet starts on another track.
    fn restart_layers(&self) {
        self.layer.lock().unwrap_or_else(PoisonError::into_inner).current = None;
    }

    /// Takes the packet loss the receiver reported for this outlet's stream: the
    /// domain decides whether its network calls for another layer.
    pub fn receiver_report(&self, fraction_lost: u8) {
        let mut state = self.layer.lock().unwrap_or_else(PoisonError::into_inner);
        state.receiver = state.receiver.report(fraction_lost);
    }

    /// Whether this outlet sends a packet of `layer` (`None`: a track without
    /// simulcast, always sent), and whether it waits for a keyframe of another layer.
    /// A switch of layer continues the sequence numbers like a switch of source.
    fn route(
        &self,
        layer: Option<SimulcastLayer>,
        keyframe: bool,
        arriving: &[SimulcastLayer],
    ) -> (Step, bool) {
        let Some(layer) = layer else {
            return (Step::Write, false);
        };
        let mut state = self.layer.lock().unwrap_or_else(PoisonError::into_inner);
        let target = SimulcastLayer::best_available(state.receiver.layer(), arriving);
        let choice = LayerChoice { current: state.current, target };
        let step = choice.step(layer, keyframe);
        if step == Step::SwitchAndWrite {
            state.current = Some(layer);
            self.switch_source();
        }
        (step, choice.waiting() && step != Step::SwitchAndWrite)
    }

    /// Announces that the next packet comes from a different published track.
    pub fn switch_source(&self) {
        self.continuity.lock().unwrap_or_else(PoisonError::into_inner).source_changed = true;
    }

    async fn write(&self, packet: &Packet, source: &Arc<Source>) {
        let Some(mut packet) = self.repack.apply(packet.clone()) else {
            debug!("malformed redundant audio packet dropped");
            return;
        };
        let (sequence_number, timestamp) = {
            let mut continuity = self.continuity.lock().unwrap_or_else(PoisonError::into_inner);
            let mut timeline = self.timeline.lock().unwrap_or_else(PoisonError::into_inner);
            let keep = Self::offset_keeping_time(&timeline, source);
            let mapped =
                continuity.map(packet.header.sequence_number, packet.header.timestamp, keep);
            if !timeline.source.as_ref().is_some_and(|current| Arc::ptr_eq(current, source)) {
                timeline.source = Some(source.clone());
            }
            timeline.offset = continuity.timestamp_offset;
            mapped
        };
        packet.header.sequence_number = sequence_number;
        packet.header.timestamp = timestamp;
        self.packets.fetch_add(1, Ordering::Relaxed);
        let size = u32::try_from(packet.payload.len()).unwrap_or(u32::MAX);
        self.octets.fetch_add(size, Ordering::Relaxed);
        if let Err(error) = self.local.write_rtp(&packet).await {
            debug!(%error, "rtp write failed");
        }
    }

    /// For a switch to `next`, the offset under which the outlet's timestamps keep
    /// meaning the same wall-clock time, so a receiver's lip sync carries on. Only
    /// within one publisher, whose streams share a clock; `None` without both
    /// streams' sender reports.
    fn offset_keeping_time(timeline: &Timeline, next: &Arc<Source>) -> Option<u32> {
        let current = timeline.source.as_ref()?;
        if Arc::ptr_eq(current, next) || !Weak::ptr_eq(&current.publisher, &next.publisher) {
            return None;
        }
        Some(timeline.clock()?.offset_for(next.clock()?))
    }
}

/// Maps a source's sequence numbers and timestamps onto one continuous series.
#[derive(Debug, Default)]
struct Continuity {
    started: bool,
    source_changed: bool,
    sequence_offset: u16,
    timestamp_offset: u32,
    /// The highest values sent so far.
    last_sequence: u16,
    last_timestamp: u32,
}

impl Continuity {
    /// How far the timestamp moves on at a source switch. Any forward step works;
    /// this one is a few frames at the usual clock rates.
    const TIMESTAMP_STEP: u32 = 9000;
    /// The furthest a kept offset may move the timestamp on at a switch: several
    /// seconds at the usual clock rates. Beyond it the reports were wrong.
    const MAX_KEPT_STEP: u32 = 1 << 20;

    /// Maps one packet. At a source switch, `keep` is the offset that keeps the
    /// timestamps on the same wall clock; it is used when it moves them forward, and
    /// otherwise the timestamps step on by [`Self::TIMESTAMP_STEP`].
    fn map(&mut self, sequence_number: u16, timestamp: u32, keep: Option<u32>) -> (u16, u32) {
        if !self.started {
            self.started = true;
            self.source_changed = false;
            self.last_sequence = sequence_number;
            self.last_timestamp = timestamp;
            return (sequence_number, timestamp);
        }
        if self.source_changed {
            self.source_changed = false;
            self.sequence_offset = self.last_sequence.wrapping_add(1).wrapping_sub(sequence_number);
            let last = self.last_timestamp;
            let forward = |offset: &u32| {
                let step = timestamp.wrapping_add(*offset).wrapping_sub(last);
                (1..=Self::MAX_KEPT_STEP).contains(&step)
            };
            self.timestamp_offset = keep
                .filter(forward)
                .unwrap_or_else(|| last.wrapping_add(Self::TIMESTAMP_STEP).wrapping_sub(timestamp));
        }

        let sequence_number = sequence_number.wrapping_add(self.sequence_offset);
        let timestamp = timestamp.wrapping_add(self.timestamp_offset);
        // "Newer" in wrapping arithmetic: less than half the range ahead. A packet
        // that arrives out of order must not pull the high-water marks back.
        if sequence_number.wrapping_sub(self.last_sequence) < 0x8000 {
            self.last_sequence = sequence_number;
        }
        if timestamp.wrapping_sub(self.last_timestamp) < 0x8000_0000 {
            self.last_timestamp = timestamp;
        }
        (sequence_number, timestamp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_source_passes_through_unchanged() {
        let mut continuity = Continuity::default();
        assert_eq!(continuity.map(100, 5_000, None), (100, 5_000));
        assert_eq!(continuity.map(101, 8_000, None), (101, 8_000));
        assert_eq!(continuity.map(102, 11_000, None), (102, 11_000));
    }

    #[test]
    fn a_new_source_continues_where_the_old_one_stopped() {
        let mut continuity = Continuity::default();
        continuity.map(60_000, 4_000_000_000, None);
        continuity.map(60_001, 4_000_003_000, None);

        // The next source starts far "behind", which a receiver would take for old packets.
        continuity.source_changed = true;
        assert_eq!(continuity.map(7, 123, None), (60_002, 4_000_012_000));
        assert_eq!(continuity.map(8, 3_123, None), (60_003, 4_000_015_000));
    }

    #[test]
    fn numbers_wrap_around_without_a_gap() {
        let mut continuity = Continuity::default();
        continuity.map(u16::MAX - 1, u32::MAX - 100, None);
        continuity.map(u16::MAX, u32::MAX - 50, None);

        continuity.source_changed = true;
        let (sequence_number, timestamp) = continuity.map(500, 500, None);
        assert_eq!(sequence_number, 0, "65535 is followed by 0");
        assert_eq!(timestamp, (u32::MAX - 50).wrapping_add(9000));
        assert_eq!(continuity.map(501, 3_500, None).0, 1);
    }

    #[test]
    fn a_reordered_packet_does_not_move_the_switch_point_back() {
        let mut continuity = Continuity::default();
        continuity.map(10, 1_000, None);
        continuity.map(12, 3_000, None);
        continuity.map(11, 2_000, None); // late

        continuity.source_changed = true;
        assert_eq!(continuity.map(900, 77, None).0, 13, "continues after the highest number sent");
    }

    #[test]
    fn a_kept_offset_carries_the_wall_clock_across_a_switch() {
        let mut continuity = Continuity::default();
        continuity.map(10, 1_000, None);
        continuity.map(11, 4_000, None);

        // The new layer's frame was captured 3_000 ticks after the last one sent.
        continuity.source_changed = true;
        let keep = 7_000u32.wrapping_sub(900_000);
        assert_eq!(continuity.map(500, 900_000, Some(keep)), (12, 7_000));
        assert_eq!(continuity.map(501, 903_000, None), (13, 10_000), "the offset stays");
    }

    #[test]
    fn a_kept_offset_that_would_go_back_or_leap_is_not_used() {
        for keep in [0u32.wrapping_sub(500), Continuity::MAX_KEPT_STEP + 1] {
            let mut continuity = Continuity::default();
            continuity.map(10, 1_000, None);
            continuity.source_changed = true;
            let (_, timestamp) = continuity.map(20, 1_000, Some(keep));
            assert_eq!(timestamp, 1_000 + Continuity::TIMESTAMP_STEP);
        }
    }

    #[test]
    fn several_switches_keep_rising() {
        let mut continuity = Continuity::default();
        let mut previous = continuity.map(5, 5, None).0;
        for source_start in [40_000u16, 3, 65_000, 12] {
            continuity.source_changed = true;
            for step in 0..3u16 {
                let (sequence_number, _) =
                    continuity.map(source_start.wrapping_add(step), u32::from(step) * 3000, None);
                assert_eq!(sequence_number, previous.wrapping_add(1));
                previous = sequence_number;
            }
        }
    }
}
