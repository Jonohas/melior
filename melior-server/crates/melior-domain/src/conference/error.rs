use super::{SessionId, TrackId};

/// A violated domain rule.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    #[error("channel id must be 1-64 characters of A-Z, a-z, 0-9, '_' or '-'")]
    InvalidChannelId,

    #[error("username {0:?} must be 1-64 characters of a-z, 0-9, '.', '_', '@' or '-'")]
    InvalidUsername(String),

    #[error("participant {0} is not in this room")]
    UnknownParticipant(SessionId),

    #[error("participant {0} is already in this room")]
    AlreadyJoined(SessionId),

    #[error("track {0} is already published")]
    DuplicateTrack(TrackId),

    #[error("participant {0} already shares a screen next to its camera")]
    TooManyVideoTracks(SessionId),

    #[error("an offer was received while a server offer is still awaiting its answer")]
    OfferDuringRenegotiation,

    #[error("an answer was received but no server offer is pending")]
    UnexpectedAnswer,

    #[error("this channel is full")]
    ChannelFull,
}
