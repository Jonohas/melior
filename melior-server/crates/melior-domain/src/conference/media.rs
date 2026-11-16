use super::TrackId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaKind {
    Audio,
    Video,
}

/// A media track a participant publishes into the room.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub id: TrackId,
    pub kind: MediaKind,
}

impl Track {
    #[must_use]
    pub fn new(id: TrackId, kind: MediaKind) -> Self {
        Self { id, kind }
    }
}
