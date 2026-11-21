//! The voice half of [`Room`]: what each participant shows the others.

use super::{DomainError, Room, SessionId, TrackSource, VoiceState};

impl Room {
    /// Takes a participant's claimed voice state. Returns the new state only if it
    /// changed. The room keeps the claim, so a screen claimed before its track
    /// arrives is shown once [`Self::reconcile_voice`] runs after the publish.
    pub fn set_voice(
        &mut self,
        id: &SessionId,
        claimed: VoiceState,
    ) -> Result<Option<VoiceState>, DomainError> {
        self.participant_mut(id)?.voice_claim = claimed;
        self.reconcile_voice(id)
    }

    /// Works out again what the others are shown of a participant's claim. Called
    /// when its tracks or its uplink change. Returns the new state only if it
    /// changed. A muted participant is not speaking. A screen counts when the room
    /// records a screen track for the participant, or when its uplink is paused:
    /// then every link is direct, the screen travels over those links only, and the
    /// SFU never sees its track.
    pub fn reconcile_voice(&mut self, id: &SessionId) -> Result<Option<VoiceState>, DomainError> {
        let participant = self.participant_mut(id)?;
        let shows_screen = participant.published.iter().any(|t| t.source == TrackSource::Screen);
        let accepted = participant.voice_claim.accepted(shows_screen || !participant.uplink_active);
        if accepted == participant.voice {
            return Ok(None);
        }
        participant.voice = accepted;
        Ok(Some(accepted))
    }

    /// A participant's voice state.
    pub fn voice(&self, id: &SessionId) -> Result<VoiceState, DomainError> {
        self.participants
            .get(id)
            .map(|p| p.voice)
            .ok_or_else(|| DomainError::UnknownParticipant(id.clone()))
    }
}
