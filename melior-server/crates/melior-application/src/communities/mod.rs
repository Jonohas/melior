//! Communities: their structure, members, roles and invites, who may enter which
//! channel, the voice roster across instances and the events their feeds carry.

mod feed;
mod memory;
mod ports;
mod presence;
mod service;
#[cfg(test)]
mod service_tests;

pub use feed::{FeedEnd, FeedFilter, FeedVerdict};
pub use memory::{InMemoryCommunityEvents, InMemoryCommunityRepository, InMemoryVoiceRoster};
pub use ports::{
    CommunityEvent, CommunityEvents, CommunityRepository, CommunityStoreError, EventsError,
    Redemption, RemovalReason, VoiceRoster,
};
pub use presence::PresenceQueue;
pub use service::{
    ChannelGrant, ChannelView, CommunitiesError, CommunityService, CommunityView, InvitePreview,
    MemberView, VoiceGrant,
};
