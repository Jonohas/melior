//! What one member's feed of a community carries: the events of the channels they
//! may view, and the end of the feed when they are no longer a member.

use super::ports::{CommunityEvent, RemovalReason};
use crate::conference::VoiceEntry;
use melior_domain::{ChannelId, UserId};
use std::collections::BTreeSet;

/// Why a feed ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedEnd {
    Kicked,
    Left,
    Deleted,
}

/// What to do with an event on one member's feed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedVerdict {
    /// Send it.
    Forward,
    /// Send it, then refresh which channels the member may view.
    ForwardAndRefresh,
    /// Not for this member.
    Drop,
    /// Tell the member why, then close the feed.
    End(FeedEnd),
}

/// The channels one member may view, and the member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedFilter {
    user_id: UserId,
    visible: BTreeSet<ChannelId>,
}

impl FeedFilter {
    #[must_use]
    pub fn new(user_id: UserId, visible: BTreeSet<ChannelId>) -> Self {
        Self { user_id, visible }
    }

    /// Decides what the feed does with `event`.
    #[must_use]
    pub fn verdict(&self, event: &CommunityEvent) -> FeedVerdict {
        match event {
            CommunityEvent::VoiceJoined(entry) | CommunityEvent::VoiceUpdated(entry) => {
                self.forward_if_visible(&entry.channel_id)
            }
            CommunityEvent::VoiceLeft { channel_id, .. }
            | CommunityEvent::Message { channel_id, .. } => self.forward_if_visible(channel_id),
            CommunityEvent::StructureChanged => FeedVerdict::ForwardAndRefresh,
            CommunityEvent::MemberRemoved { user_id, reason } if *user_id == self.user_id => {
                FeedVerdict::End(match reason {
                    RemovalReason::Kicked => FeedEnd::Kicked,
                    RemovalReason::Left => FeedEnd::Left,
                })
            }
            CommunityEvent::MemberJoined { .. } | CommunityEvent::MemberRemoved { .. } => {
                FeedVerdict::Forward
            }
            CommunityEvent::CommunityDeleted => FeedVerdict::End(FeedEnd::Deleted),
        }
    }

    /// The part of a roster the member may see.
    #[must_use]
    pub fn snapshot(&self, roster: Vec<VoiceEntry>) -> Vec<VoiceEntry> {
        roster.into_iter().filter(|entry| self.visible.contains(&entry.channel_id)).collect()
    }

    fn forward_if_visible(&self, channel_id: &ChannelId) -> FeedVerdict {
        if self.visible.contains(channel_id) {
            return FeedVerdict::Forward;
        }
        FeedVerdict::Drop
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melior_domain::{ChatMessage, ChatText, MessageId, SessionId, VoiceState};

    fn channel(name: &str) -> ChannelId {
        ChannelId::parse(name).unwrap()
    }

    fn filter() -> FeedFilter {
        FeedFilter::new(UserId::new("sub-bob"), BTreeSet::from([channel("open")]))
    }

    fn entry(channel_id: &str) -> VoiceEntry {
        VoiceEntry {
            channel_id: channel(channel_id),
            session_id: SessionId::generate(),
            user_id: UserId::new("sub-alice"),
            username: "alice".into(),
            voice: VoiceState::default(),
        }
    }

    fn message(channel_id: &str) -> CommunityEvent {
        CommunityEvent::Message {
            channel_id: channel(channel_id),
            message: ChatMessage {
                id: MessageId::new(1),
                channel_id: channel(channel_id),
                author: "alice".into(),
                content: ChatText::parse("hi").unwrap().into(),
                sent_at_ms: 0,
            },
        }
    }

    #[test]
    fn voice_and_messages_pass_only_for_visible_channels() {
        let f = filter();
        assert_eq!(f.verdict(&CommunityEvent::VoiceJoined(entry("open"))), FeedVerdict::Forward);
        assert_eq!(f.verdict(&CommunityEvent::VoiceUpdated(entry("hidden"))), FeedVerdict::Drop);
        let left = |name: &str| CommunityEvent::VoiceLeft {
            channel_id: channel(name),
            session_id: SessionId::generate(),
        };
        assert_eq!(f.verdict(&left("open")), FeedVerdict::Forward);
        assert_eq!(f.verdict(&left("hidden")), FeedVerdict::Drop);
        assert_eq!(f.verdict(&message("open")), FeedVerdict::Forward);
        assert_eq!(f.verdict(&message("hidden")), FeedVerdict::Drop);
    }

    #[test]
    fn a_structure_change_is_sent_and_refreshes_the_filter() {
        assert_eq!(
            filter().verdict(&CommunityEvent::StructureChanged),
            FeedVerdict::ForwardAndRefresh
        );
    }

    #[test]
    fn the_feed_ends_for_the_removed_member_and_for_everyone_on_deletion() {
        let f = filter();
        let removed =
            |id: &str, reason| CommunityEvent::MemberRemoved { user_id: UserId::new(id), reason };
        assert_eq!(
            f.verdict(&removed("sub-bob", RemovalReason::Kicked)),
            FeedVerdict::End(FeedEnd::Kicked)
        );
        assert_eq!(
            f.verdict(&removed("sub-bob", RemovalReason::Left)),
            FeedVerdict::End(FeedEnd::Left)
        );
        assert_eq!(
            f.verdict(&removed("sub-carol", RemovalReason::Kicked)),
            FeedVerdict::Forward,
            "the others learn that the member list changed"
        );
        assert_eq!(
            f.verdict(&CommunityEvent::MemberJoined { user_id: UserId::new("sub-dave") }),
            FeedVerdict::Forward
        );
        assert_eq!(
            f.verdict(&CommunityEvent::CommunityDeleted),
            FeedVerdict::End(FeedEnd::Deleted)
        );
    }

    #[test]
    fn the_snapshot_holds_only_visible_channels() {
        let snapshot = filter().snapshot(vec![entry("open"), entry("hidden")]);
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].channel_id, channel("open"));
    }
}
