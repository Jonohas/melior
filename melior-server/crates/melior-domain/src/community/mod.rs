//! Communities (what the UI calls servers): members, roles, categories, channels,
//! permission overrides and invites, and the rules on who may do what where.

mod channels;
mod error;
mod ids;
mod invite;
mod model;
mod permissions;
mod roles;
mod standing;
#[cfg(test)]
mod standing_tests;
mod state;
#[cfg(test)]
mod state_tests;
mod text;

pub use channels::{CategoryChanges, ChannelChanges, ChannelDraft};
pub use error::{CommunityError, Refusal, Rejected};
pub(crate) use ids::uuid_id;
pub use ids::{CategoryId, CommunityId, InviteCode, RoleId};
pub use invite::{Invite, InviteSettings};
pub use model::{
    Category, Channel, ChannelKind, Community, Member, Override, OverrideTarget, Role,
};
pub use permissions::Permissions;
pub use roles::{RoleChanges, RoleDraft};
pub use standing::{Rank, Standing};
pub use state::CommunityState;
pub use text::{Name, Topic};
