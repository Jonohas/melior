use super::TrackId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaKind {
    Audio,
    Video,
}

/// Where a published track comes from: what the participant is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrackSource {
    /// The participant's camera and microphone.
    Camera,
    /// The participant's screen: its picture and, when the sharer included it, its sound.
    Screen,
}

/// A media track a participant publishes into the room.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub id: TrackId,
    pub kind: MediaKind,
    /// Set by the room when the track is published, from what the participant already
    /// publishes: a client says nothing about it.
    pub source: TrackSource,
}

impl Track {
    /// A track as the media plane reports it. Its source is decided by
    /// [`Room::publish`](super::Room::publish).
    #[must_use]
    pub fn new(id: TrackId, kind: MediaKind) -> Self {
        Self { id, kind, source: TrackSource::Camera }
    }
}
