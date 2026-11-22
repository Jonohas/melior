//! The mesh use cases: carrying out what the room decided about links, routes and
//! uplinks, and relaying the messages of direct connections.

use super::{ConferenceService, Session};
use crate::conference::ports::Signal;
use melior_domain::{
    ChannelId, Effects, IceCandidate, KeyEnvelope, LinkTimeout, Pair, Room, Sdp, SessionId,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// How long a link may stay in `ToDirect` before the direct path counts as failed.
pub(super) const DIRECT_LINK_TIMEOUT: Duration = Duration::from_secs(10);

impl ConferenceService {
    /// Carries out what a room change asks for, with the room's lock held: tells the
    /// clients, changes what the SFU sends and renegotiates each affected session once,
    /// and arms the `ToDirect` timeouts. Nothing here waits on a client.
    pub(super) async fn carry_out(&self, room: &mut Room, effects: Effects) {
        self.send_notices(&effects);
        let mut changed = Vec::new();
        for (subscriber_id, tracks) in &effects.subscribe {
            let Some(subscriber) = self.session(subscriber_id) else {
                continue;
            };
            for track in tracks {
                if Self::subscribe(room, &subscriber, &track.id).await {
                    changed.push(subscriber_id.clone());
                }
            }
        }
        for (subscriber_id, track_ids) in &effects.unsubscribe {
            let Some(subscriber) = self.session(subscriber_id) else {
                continue;
            };
            for track_id in track_ids {
                if let Err(error) = subscriber.media.unsubscribe(track_id).await {
                    warn!(session = %subscriber_id, track = %track_id, %error, "unsubscribe failed");
                }
            }
            changed.push(subscriber_id.clone());
        }
        changed.sort();
        changed.dedup();
        for session_id in changed {
            if let Some(session) = self.session(&session_id) {
                self.renegotiate(room, &session).await;
            }
        }
        for (session_id, _) in &effects.uplinks {
            self.reconcile_voice(room, session_id);
        }
        for timeout in effects.timeouts {
            self.arm_timeout(room.id(), timeout);
        }
    }

    fn send_notices(&self, effects: &Effects) {
        for notice in &effects.routes {
            self.send_to(
                &notice.to,
                Signal::Route { peer: notice.peer.clone(), path: notice.path, epoch: notice.epoch },
            );
        }
        for close in &effects.link_closes {
            self.send_to(&close.to, Signal::LinkClose { peer: close.peer.clone() });
        }
        for (session_id, active) in &effects.uplinks {
            self.send_to(session_id, Signal::Uplink { active: *active });
        }
    }

    fn send_to(&self, session_id: &SessionId, signal: Signal) {
        if let Some(session) = self.session(session_id) {
            session.signals.send(signal);
        }
    }

    /// Runs [`Self::link_timeout`] once [`DIRECT_LINK_TIMEOUT`] has passed.
    fn arm_timeout(&self, channel_id: &ChannelId, timeout: LinkTimeout) {
        let Some(service) = self.this.upgrade() else {
            return;
        };
        let channel_id = channel_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(DIRECT_LINK_TIMEOUT).await;
            service.link_timeout(&channel_id, &timeout.pair, timeout.epoch).await;
        });
    }

    /// The `ToDirect` timeout for `epoch` fired. If the link still waits for its
    /// reports, the room is pinned to forwarding. A stale epoch, a vanished link or a
    /// vanished room does nothing.
    #[tracing::instrument(name = "conference.link_timeout", skip_all, fields(channel = %channel_id, epoch))]
    pub async fn link_timeout(&self, channel_id: &ChannelId, pair: &Pair, epoch: u64) {
        let Some(cell) = super::lock(&self.rooms).get(channel_id).cloned() else {
            return;
        };
        let mut room = cell.lock().await;
        let effects = room.link_timeout(pair, epoch);
        if effects.is_empty() {
            return;
        }
        info!("direct link did not come up in time; the room is forwarded");
        self.carry_out(&mut room, effects).await;
    }

    /// A participant plays `peer`'s media over the path of route `epoch`.
    /// A stale or foreign report changes nothing.
    #[tracing::instrument(name = "conference.route_live", skip_all, fields(session = %session_id))]
    pub async fn route_live(
        &self,
        session_id: &SessionId,
        peer: &SessionId,
        epoch: u64,
    ) -> Result<(), super::ConferenceError> {
        let session = self.require_session(session_id)?;
        let mut room = session.room.lock().await;
        let effects = room.route_live(session_id, peer, epoch);
        self.carry_out(&mut room, effects).await;
        Ok(())
    }

    /// A participant's direct connection to `peer` failed. Applies the failure handling
    /// of a timed-out link; ignored while no direct connection exists.
    #[tracing::instrument(name = "conference.link_failed", skip_all, fields(session = %session_id))]
    pub async fn link_failed(
        &self,
        session_id: &SessionId,
        peer: &SessionId,
    ) -> Result<(), super::ConferenceError> {
        let session = self.require_session(session_id)?;
        let mut room = session.room.lock().await;
        let effects = room.link_failed(session_id, peer);
        if effects.is_empty() {
            debug!(session = %session_id, peer = %peer, "link failure ignored");
            return Ok(());
        }
        info!(session = %session_id, peer = %peer, "direct link failed; the room is forwarded");
        self.carry_out(&mut room, effects).await;
        Ok(())
    }

    /// Relays an offer for the direct connection between `from` and `to`.
    pub async fn relay_peer_offer(&self, from: &SessionId, to: &SessionId, sdp: Sdp) {
        self.relay(from, to, Room::may_relay, |from| Signal::PeerOffer { from, sdp }).await;
    }

    /// Relays an answer for the direct connection between `from` and `to`.
    pub async fn relay_peer_answer(&self, from: &SessionId, to: &SessionId, sdp: Sdp) {
        self.relay(from, to, Room::may_relay, |from| Signal::PeerAnswer { from, sdp }).await;
    }

    /// Relays an ICE candidate for the direct connection between `from` and `to`.
    pub async fn relay_peer_ice(&self, from: &SessionId, to: &SessionId, candidate: IceCandidate) {
        self.relay(from, to, Room::may_relay, |from| Signal::PeerIce { from, candidate }).await;
    }

    /// Relays an end-to-end key message to another participant of the sender's room,
    /// on any path. The server cannot read it; see [`KeyEnvelope`].
    pub async fn relay_key(&self, from: &SessionId, to: &SessionId, envelope: KeyEnvelope) {
        self.relay(from, to, Room::may_relay_keys, |from| Signal::Key { from, envelope }).await;
    }

    /// Forwards a peer message only if the room's `allowed` rule lets it go from `from`
    /// to `to` (for direct-link signalling: `to` is in the sender's room and their link
    /// has a direct connection). Anything else is dropped and logged at debug level,
    /// never answered with an error: the client tears down everything on `error`, and a
    /// late candidate after `link_close` is a normal race.
    async fn relay(
        &self,
        from: &SessionId,
        to: &SessionId,
        allowed: fn(&Room, &SessionId, &SessionId) -> bool,
        message: impl FnOnce(SessionId) -> Signal,
    ) {
        let Some(sender) = self.session(from) else {
            debug!(session = %from, "relay from an unknown session dropped");
            return;
        };
        let room = sender.room.lock().await;
        if !allowed(&room, from, to) {
            debug!(session = %from, peer = %to, "relay the room does not allow dropped");
            return;
        }
        let Some(target): Option<Arc<Session>> = self.session(to) else {
            debug!(session = %from, peer = %to, "relay to a vanished session dropped");
            return;
        };
        target.signals.send(message(from.clone()));
    }
}
