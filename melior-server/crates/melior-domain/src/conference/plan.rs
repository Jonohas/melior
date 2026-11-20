//! How a channel's media settings and the instance's mesh policy decide the path
//! and the capacity of its room.

use super::{MediaPath, MeshPolicy};

/// What a channel asks for: direct, forwarded, or switching with the room's size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MediaMode {
    /// Direct while the room is small, forwarded once it grows (the hysteresis of
    /// [`RoomTarget`](super::RoomTarget)).
    #[default]
    Auto,
    /// Always browser to browser.
    PeerToPeer,
    /// Always through the SFU.
    Server,
}

impl MediaMode {
    /// The wire name: `auto`, `peer_to_peer` or `server`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::PeerToPeer => "peer_to_peer",
            Self::Server => "server",
        }
    }

    /// The inverse of [`Self::as_str`]; anything else is `None`.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "auto" => Some(Self::Auto),
            "peer_to_peer" => Some(Self::PeerToPeer),
            "server" => Some(Self::Server),
            _ => None,
        }
    }
}

/// A channel's media settings. The user limit is validated by the channel (2 to 99).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ChannelMedia {
    pub mode: MediaMode,
    pub user_limit: Option<u8>,
}

/// Why the plan differs from what the channel asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaWarning {
    /// Peer-to-peer was asked for, but this instance runs no mesh: forwarded.
    MeshOff,
    /// The user limit is above the mesh limit: the mesh limit caps the room.
    LimitClamped,
}

impl MediaWarning {
    /// The wire name: `mesh_off` or `limit_clamped`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MeshOff => "mesh_off",
            Self::LimitClamped => "limit_clamped",
        }
    }
}

/// What a room does with its media: a forced path or switching, and how many
/// participants it admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomPlan {
    forced: Option<MediaPath>,
    capacity: Option<usize>,
    effective: MediaMode,
    warning: Option<MediaWarning>,
}

impl RoomPlan {
    /// Resolves a channel's settings against this instance's mesh policy.
    #[must_use]
    pub fn resolve(media: ChannelMedia, policy: MeshPolicy) -> Self {
        let limit = media.user_limit.map(usize::from);
        match media.mode {
            MediaMode::Server => Self::forwarded(limit, None),
            MediaMode::PeerToPeer if !policy.is_on() => {
                Self::forwarded(limit, Some(MediaWarning::MeshOff))
            }
            MediaMode::PeerToPeer => Self::peer_to_peer(limit, policy.limit()),
            MediaMode::Auto if limit.is_some_and(|l| policy.is_on() && l <= policy.limit()) => {
                Self::peer_to_peer(limit, policy.limit())
            }
            MediaMode::Auto if !policy.is_on() => Self::forwarded(limit, None),
            MediaMode::Auto => {
                Self { forced: None, capacity: limit, effective: MediaMode::Auto, warning: None }
            }
        }
    }

    fn forwarded(capacity: Option<usize>, warning: Option<MediaWarning>) -> Self {
        Self { forced: Some(MediaPath::Forwarded), capacity, effective: MediaMode::Server, warning }
    }

    fn peer_to_peer(limit: Option<usize>, mesh_limit: usize) -> Self {
        let capacity = limit.unwrap_or(mesh_limit).min(mesh_limit);
        let warning = limit.is_some_and(|l| l > mesh_limit).then_some(MediaWarning::LimitClamped);
        Self {
            forced: Some(MediaPath::Direct),
            capacity: Some(capacity),
            effective: MediaMode::PeerToPeer,
            warning,
        }
    }

    /// The path the room always takes, or `None` when it switches with its size.
    #[must_use]
    pub fn forced(&self) -> Option<MediaPath> {
        self.forced
    }

    /// How many participants the room admits; `None` is unlimited.
    #[must_use]
    pub fn capacity(&self) -> Option<usize> {
        self.capacity
    }

    /// What the room actually does: `Auto` only while it switches with its size.
    #[must_use]
    pub fn effective_mode(&self) -> MediaMode {
        self.effective
    }

    #[must_use]
    pub fn warning(&self) -> Option<MediaWarning> {
        self.warning
    }
}

impl Default for RoomPlan {
    /// An `Auto` channel without a limit on an instance without mesh.
    fn default() -> Self {
        Self::resolve(ChannelMedia::default(), MeshPolicy::OFF)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(mode: MediaMode, user_limit: Option<u8>, mesh: usize) -> RoomPlan {
        RoomPlan::resolve(ChannelMedia { mode, user_limit }, MeshPolicy::new(mesh))
    }

    #[test]
    fn server_is_always_forwarded_with_the_user_limit_as_capacity() {
        for mesh in [0, 4] {
            let plan = plan(MediaMode::Server, Some(10), mesh);
            assert_eq!(plan.forced(), Some(MediaPath::Forwarded));
            assert_eq!(plan.capacity(), Some(10));
            assert_eq!(plan.effective_mode(), MediaMode::Server);
            assert_eq!(plan.warning(), None);
        }
        assert_eq!(plan(MediaMode::Server, None, 4).capacity(), None);
    }

    #[test]
    fn peer_to_peer_with_mesh_on_is_direct_and_capped_by_the_mesh_limit() {
        let unlimited = plan(MediaMode::PeerToPeer, None, 4);
        assert_eq!(unlimited.forced(), Some(MediaPath::Direct));
        assert_eq!(unlimited.capacity(), Some(4), "the mesh limit when no user limit is set");
        assert_eq!(unlimited.effective_mode(), MediaMode::PeerToPeer);
        assert_eq!(unlimited.warning(), None);

        let smaller = plan(MediaMode::PeerToPeer, Some(3), 4);
        assert_eq!(smaller.capacity(), Some(3));
        assert_eq!(smaller.warning(), None);
    }

    #[test]
    fn peer_to_peer_above_the_mesh_limit_is_clamped_with_a_warning() {
        let plan = plan(MediaMode::PeerToPeer, Some(6), 4);
        assert_eq!(plan.capacity(), Some(4));
        assert_eq!(plan.warning(), Some(MediaWarning::LimitClamped));
    }

    #[test]
    fn peer_to_peer_with_mesh_off_is_forwarded_with_a_warning() {
        let plan = plan(MediaMode::PeerToPeer, Some(3), 0);
        assert_eq!(plan.forced(), Some(MediaPath::Forwarded));
        assert_eq!(plan.capacity(), Some(3));
        assert_eq!(plan.effective_mode(), MediaMode::Server);
        assert_eq!(plan.warning(), Some(MediaWarning::MeshOff));
    }

    #[test]
    fn auto_within_the_mesh_limit_is_peer_to_peer() {
        let plan = plan(MediaMode::Auto, Some(4), 4);
        assert_eq!(plan.forced(), Some(MediaPath::Direct));
        assert_eq!(plan.capacity(), Some(4));
        assert_eq!(plan.effective_mode(), MediaMode::PeerToPeer);
    }

    #[test]
    fn auto_otherwise_switches_with_the_user_limit_as_capacity() {
        for limit in [None, Some(5), Some(99)] {
            let plan = plan(MediaMode::Auto, limit, 4);
            assert_eq!(plan.forced(), None);
            assert_eq!(plan.capacity(), limit.map(usize::from));
            assert_eq!(plan.effective_mode(), MediaMode::Auto);
            assert_eq!(plan.warning(), None);
        }
    }

    #[test]
    fn auto_with_mesh_off_is_reported_as_forwarded() {
        let plan = plan(MediaMode::Auto, Some(3), 0);
        assert_eq!(plan.forced(), Some(MediaPath::Forwarded));
        assert_eq!(plan.effective_mode(), MediaMode::Server);
        assert_eq!(plan.capacity(), Some(3));
    }

    #[test]
    fn mode_and_warning_wire_names_round_trip() {
        for mode in [MediaMode::Auto, MediaMode::PeerToPeer, MediaMode::Server] {
            assert_eq!(MediaMode::parse(mode.as_str()), Some(mode));
        }
        assert_eq!(MediaMode::parse("p2p"), None);
        assert_eq!(MediaWarning::MeshOff.as_str(), "mesh_off");
        assert_eq!(MediaWarning::LimitClamped.as_str(), "limit_clamped");
    }
}
