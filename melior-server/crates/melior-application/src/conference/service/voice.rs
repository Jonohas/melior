//! Voice states, capacity and presence: what the rest of a community learns about
//! a room.

use super::{ConferenceError, ConferenceService, Session, lock};
use crate::conference::ports::Signal;
use crate::conference::presence::{PresenceUpdate, VoiceEntry};
use melior_domain::{ChannelId, CommunityId, Room, RoomPlan, SessionId, VoiceState};
use tracing::debug;

impl ConferenceService {
    /// A participant claims a voice state. Over the rate limit the claim is
    /// dropped; otherwise the room decides what it accepts, and a change goes to
    /// everyone in the room (the participant included) and to the community.
    #[tracing::instrument(name = "conference.voice_state", skip_all, fields(session = %session_id))]
    pub async fn set_voice(
        &self,
        session_id: &SessionId,
        claimed: VoiceState,
    ) -> Result<(), ConferenceError> {
        let session = self.require_session(session_id)?;
        let now_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        if !lock(&session.voice_limit).allows(now_ms) {
            debug!(session = %session_id, "voice state over the rate limit dropped");
            return Ok(());
        }
        let mut room = session.room.lock().await;
        if let Some(voice) = room.set_voice(session_id, claimed)? {
            self.announce_voice(&room, &session, voice);
        }
        Ok(())
    }

    /// Re-evaluates what a participant shows after its tracks or its uplink
    /// changed, with the room's lock held, and announces a change.
    pub(super) fn reconcile_voice(&self, room: &mut Room, session_id: &SessionId) {
        let Some(session) = self.session(session_id) else {
            return;
        };
        match room.reconcile_voice(session_id) {
            Ok(Some(voice)) => self.announce_voice(room, &session, voice),
            Ok(None) => {}
            Err(error) => debug!(session = %session_id, %error, "voice not reconciled"),
        }
    }

    /// Tells everyone in the room (the participant included) and the community.
    fn announce_voice(&self, room: &Room, session: &Session, voice: VoiceState) {
        for participant in room.roster() {
            if let Some(target) = self.session(&participant.session_id) {
                target.signals.send(Signal::VoiceState { session_id: session.id.clone(), voice });
            }
        }
        self.report(session, voice, PresenceUpdate::updated);
    }

    /// Reports the participant again to the presence store, so that its entry
    /// outlives the store's expiry while the participant stays. Called on every
    /// re-check of its connection.
    pub async fn refresh_presence(&self, session_id: &SessionId) -> Result<(), ConferenceError> {
        let session = self.require_session(session_id)?;
        let voice = session.room.lock().await.voice(session_id)?;
        self.report(&session, voice, PresenceUpdate::refreshed);
        Ok(())
    }

    /// Whether a room on this instance already holds as many participants as `plan`
    /// admits. A room that does not exist here is empty. Asked before a WebSocket
    /// is accepted; the join itself checks again.
    pub async fn is_full(&self, channel_id: &ChannelId, plan: RoomPlan) -> bool {
        let Some(capacity) = plan.capacity() else {
            return false;
        };
        let Some(cell) = lock(&self.rooms).get(channel_id).cloned() else {
            return false;
        };
        cell.lock().await.len() >= capacity
    }

    /// Queues a presence report for a participant of a community. Never waits.
    pub(super) fn report(
        &self,
        session: &Session,
        voice: VoiceState,
        update: fn(CommunityId, VoiceEntry) -> PresenceUpdate,
    ) {
        let Some(context) = &session.presence else {
            return;
        };
        let entry = VoiceEntry {
            channel_id: session.channel_id.clone(),
            session_id: session.id.clone(),
            user_id: context.user_id.clone(),
            username: session.username.clone(),
            voice,
        };
        self.presence.report(update(context.community_id.clone(), entry));
    }
}
