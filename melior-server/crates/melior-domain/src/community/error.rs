/// Input that does not fit a community rule. The wording reaches the person who sent it.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum CommunityError {
    #[error("a name is 1 to 100 characters, without control characters")]
    InvalidName,

    #[error("a topic is at most 1024 characters")]
    InvalidTopic,

    #[error("a colour is a number from 0 to 16777215 (0xFFFFFF)")]
    InvalidColor,

    #[error("unknown permission {0:?}")]
    UnknownPermission(String),

    #[error("a user limit is 2 to 99")]
    InvalidUserLimit,

    #[error("an invite's maximum number of uses is 1 to 100")]
    InvalidMaxUses,

    #[error("an invite lasts 1800, 3600, 21600, 43200, 86400 or 604800 seconds, or forever")]
    InvalidMaxAge,

    #[error("a text channel has no media mode and no user limit")]
    TextChannelMedia,

    #[error("a peer-to-peer channel holds at most {0} people on this server")]
    PeerToPeerLimit(usize),

    #[error("the @everyone role cannot be renamed, moved or deleted")]
    EveryoneRole,

    #[error("a role's position is at least 1")]
    InvalidPosition,

    #[error("unknown role {0}")]
    UnknownRole(String),

    #[error("unknown category {0}")]
    UnknownCategory(String),

    #[error("an override names a role or a member, each at most once")]
    InvalidOverride,

    #[error("unknown channel")]
    UnknownChannel,
}

/// Why a member may not do something. The wording reaches that member.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    #[error("you need the {0} permission")]
    Missing(&'static str),

    #[error("that role is not below your highest role")]
    RoleNotBelow,

    #[error("you cannot grant permissions you do not have")]
    ExceedsOwnPermissions,

    #[error("only the owner may do that")]
    OwnerOnly,

    #[error("the owner cannot leave; delete the server instead")]
    OwnerCannotLeave,

    #[error("the owner cannot be kicked")]
    CannotKickOwner,

    #[error("that member's highest role is not below yours")]
    TargetOutranks,

    #[error("this is not a voice channel")]
    NotVoice,
}

/// Why a change to a community is not made: the input is invalid, or the member
/// may not make it.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum Rejected {
    #[error(transparent)]
    Invalid(#[from] CommunityError),

    #[error(transparent)]
    Refused(#[from] Refusal),
}
