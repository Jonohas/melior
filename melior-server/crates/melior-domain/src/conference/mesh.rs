//! The room-wide choice between direct (mesh) and forwarded (SFU) media.

/// The path media takes between two participants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaPath {
    /// Browser to browser.
    Direct,
    /// Through the SFU.
    Forwarded,
}

/// How large a room may be and still run as a mesh. A limit of 0 turns mesh off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshPolicy {
    limit: usize,
}

impl MeshPolicy {
    /// Mesh off: every room is forwarded, exactly as without this feature.
    pub const OFF: Self = Self { limit: 0 };

    /// The limit is validated where it is read (the configuration), not here.
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self { limit }
    }

    #[must_use]
    pub fn limit(self) -> usize {
        self.limit
    }

    #[must_use]
    pub fn is_on(self) -> bool {
        self.limit > 0
    }
}

/// The path a room as a whole wants, with hysteresis: a room that has grown past the
/// limit only returns to direct once it is one participant below it, so it does not
/// flap at the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomTarget {
    policy: MeshPolicy,
    path: MediaPath,
    pinned: bool,
}

impl RoomTarget {
    /// An empty room starts direct when mesh is on.
    #[must_use]
    pub fn new(policy: MeshPolicy) -> Self {
        let path = if policy.is_on() { MediaPath::Direct } else { MediaPath::Forwarded };
        Self { policy, path, pinned: false }
    }

    #[must_use]
    pub fn path(&self) -> MediaPath {
        self.path
    }

    /// True after a direct link failed, until the next join or leave.
    #[must_use]
    pub fn is_pinned(&self) -> bool {
        self.pinned
    }

    /// Re-evaluates the target after a join or a leave. Clears the pin.
    pub fn evaluate(&mut self, participants: usize) -> MediaPath {
        self.pinned = false;
        let limit = self.policy.limit();
        if limit == 0 || participants > limit {
            self.path = MediaPath::Forwarded;
        } else if participants < limit {
            self.path = MediaPath::Direct;
        }
        self.path
    }

    /// A direct link failed: forward the whole room until the next join or leave.
    pub fn pin_forwarded(&mut self) {
        self.pinned = true;
        self.path = MediaPath::Forwarded;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(limit: usize) -> RoomTarget {
        RoomTarget::new(MeshPolicy::new(limit))
    }

    #[test]
    fn an_empty_room_starts_direct_only_when_mesh_is_on() {
        assert_eq!(target(4).path(), MediaPath::Direct);
        assert_eq!(target(0).path(), MediaPath::Forwarded);
        assert_eq!(RoomTarget::new(MeshPolicy::OFF).path(), MediaPath::Forwarded);
    }

    #[test]
    fn mesh_off_is_always_forwarded() {
        let mut target = target(0);
        for count in 0..10 {
            assert_eq!(target.evaluate(count), MediaPath::Forwarded);
        }
    }

    #[test]
    fn growing_past_the_limit_forwards_and_shrinking_below_it_returns_to_direct() {
        let mut target = target(4);
        assert_eq!(target.evaluate(4), MediaPath::Direct, "at the limit nothing changes");
        assert_eq!(target.evaluate(5), MediaPath::Forwarded);
        assert_eq!(target.evaluate(4), MediaPath::Forwarded, "hysteresis at exactly the limit");
        assert_eq!(target.evaluate(3), MediaPath::Direct);
        assert_eq!(target.evaluate(4), MediaPath::Direct, "hysteresis the other way");
    }

    #[test]
    fn a_pin_holds_until_the_next_evaluation() {
        let mut target = target(4);
        target.pin_forwarded();
        assert!(target.is_pinned());
        assert_eq!(target.path(), MediaPath::Forwarded);

        assert_eq!(target.evaluate(2), MediaPath::Direct);
        assert!(!target.is_pinned());
    }

    #[test]
    fn a_pin_at_the_limit_keeps_forwarding_after_it_is_cleared() {
        let mut target = target(4);
        target.pin_forwarded();
        assert_eq!(target.evaluate(4), MediaPath::Forwarded);
    }
}
