//! What a participant shows the others about itself: muted, camera, screen, speaking.

use crate::chat::SendLimit;

/// A participant's voice state. All false on join.
// Four independent flags, exactly as clients show them; an enum would not fit.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct VoiceState {
    pub muted: bool,
    pub camera: bool,
    pub screen: bool,
    pub speaking: bool,
}

impl VoiceState {
    /// The claim as the room accepts it: a muted participant is not speaking, and a
    /// screen counts only when `may_share_screen`.
    #[must_use]
    pub fn accepted(self, may_share_screen: bool) -> Self {
        Self {
            muted: self.muted,
            camera: self.camera,
            screen: self.screen && may_share_screen,
            speaking: self.speaking && !self.muted,
        }
    }
}

/// How many voice-state updates one participant may send: ten per second, the
/// rest are dropped. Speaking detection sends one per change, so this is far
/// above what a well-behaved client needs.
#[derive(Debug, Clone)]
pub struct VoiceLimit(SendLimit);

impl VoiceLimit {
    pub const PER_SECOND: usize = 10;

    #[must_use]
    pub fn standard() -> Self {
        Self(SendLimit::new(Self::PER_SECOND, 1000))
    }

    /// Records an update at `now_ms`; false when it is over the limit and dropped.
    pub fn allows(&mut self, now_ms: u64) -> bool {
        self.0.record(now_ms).is_ok()
    }
}

impl Default for VoiceLimit {
    fn default() -> Self {
        Self::standard()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::fn_params_excessive_bools)]
    fn state(muted: bool, camera: bool, screen: bool, speaking: bool) -> VoiceState {
        VoiceState { muted, camera, screen, speaking }
    }

    #[test]
    fn everything_is_off_on_join() {
        assert_eq!(VoiceState::default(), state(false, false, false, false));
    }

    #[test]
    fn a_muted_participant_is_not_speaking() {
        assert_eq!(
            state(true, false, false, true).accepted(true),
            state(true, false, false, false)
        );
        assert_eq!(
            state(false, false, false, true).accepted(true),
            state(false, false, false, true)
        );
    }

    #[test]
    fn a_screen_counts_only_when_one_may_be_shown() {
        assert_eq!(
            state(false, true, true, false).accepted(false),
            state(false, true, false, false)
        );
        assert_eq!(state(false, true, true, false).accepted(true), state(false, true, true, false));
    }

    #[test]
    fn the_eleventh_update_within_a_second_is_dropped() {
        let mut limit = VoiceLimit::standard();
        for ms in 0..10 {
            assert!(limit.allows(ms * 10));
        }
        assert!(!limit.allows(500));
        assert!(limit.allows(1000), "the window slides");
    }
}
