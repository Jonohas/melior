use super::ports::{ChatRepository, ChatStoreError};
use crate::communities::{CommunitiesError, CommunityEvent, CommunityService};
use crate::conference::{ConferenceError, ConferenceService, Signal};
use melior_domain::{
    ChannelId, ChatError, ChatMessage, ChatText, MessageId, Permissions, SendLimit, SessionId,
    User, UserId,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;
use tracing::warn;

/// How many messages a history page holds unless the caller asks for fewer.
const DEFAULT_PAGE: usize = 50;
const MAX_PAGE: usize = 100;

#[derive(thiserror::Error, Debug)]
pub enum ChatFailure {
    /// The message broke a chat rule; the wording is fit for the sender.
    #[error(transparent)]
    Rejected(#[from] ChatError),

    #[error(transparent)]
    Conference(#[from] ConferenceError),

    #[error(transparent)]
    Store(#[from] ChatStoreError),

    /// The channel is unknown, of the wrong kind, or closed to the user.
    #[error(transparent)]
    Community(#[from] CommunitiesError),
}

/// One page of a room's history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatPage {
    /// Oldest first.
    pub messages: Vec<ChatMessage>,
    /// Whether older messages exist than the oldest one here.
    pub has_more: bool,
}

/// The chat use cases: saving what a participant says and telling the room, and
/// reading what was said before.
pub struct ChatService {
    conference: Arc<ConferenceService>,
    messages: Arc<dyn ChatRepository>,
    communities: Arc<CommunityService>,
    limits: Mutex<HashMap<SessionId, SendLimit>>,
    /// The limits of text channel posts, per user, since a user has no session there.
    post_limits: Mutex<HashMap<UserId, SendLimit>>,
    started: Instant,
}

impl ChatService {
    pub fn new(
        conference: Arc<ConferenceService>,
        messages: Arc<dyn ChatRepository>,
        communities: Arc<CommunityService>,
    ) -> Self {
        Self {
            conference,
            messages,
            communities,
            limits: Mutex::new(HashMap::new()),
            post_limits: Mutex::new(HashMap::new()),
            started: Instant::now(),
        }
    }

    /// Saves what a participant said and tells everyone in its room, the author
    /// included. The author's copy carries `client_id`, which the client chose to
    /// recognise its own message. `permissions` is what the participant may do in
    /// the channel; sending needs `send_messages`.
    #[tracing::instrument(name = "chat.send", skip_all, fields(session = %session_id))]
    pub async fn send(
        &self,
        session_id: &SessionId,
        client_id: Option<String>,
        raw_text: &str,
        permissions: Permissions,
    ) -> Result<(), ChatFailure> {
        let (channel_id, author) = self.conference.whereabouts(session_id).await?;
        ChatError::require_send(permissions)?;
        let text = ChatText::parse(raw_text)?;
        self.check_limit(session_id)?;
        let message = self.messages.append(&channel_id, &author, &text).await?;
        let for_others = Signal::Chat { message: message.clone(), client_id: None };
        let for_sender = Signal::Chat { message, client_id };
        self.conference.tell_room(session_id, for_sender, for_others).await?;
        Ok(())
    }

    /// Posts to a text channel: needs `view_channel` and `send_messages` there. The
    /// message is saved and published on the community's feed.
    #[tracing::instrument(name = "chat.post", skip_all, fields(channel = %channel_id, user = %user.username))]
    pub async fn post(
        &self,
        channel_id: &ChannelId,
        user: &User,
        raw_text: &str,
    ) -> Result<ChatMessage, ChatFailure> {
        let grant = self.communities.text_grant(channel_id, user).await?;
        let text = ChatText::parse(raw_text)?;
        self.check_post_limit(&user.id)?;
        let message = self.messages.append(channel_id, &user.username, &text).await?;
        let event =
            CommunityEvent::Message { channel_id: channel_id.clone(), message: message.clone() };
        if let Err(error) = self.communities.events().publish(&grant.community_id, event).await {
            warn!(%error, channel = %channel_id, "chat message not published");
        }
        Ok(message)
    }

    /// A page of a channel's history for a user who may read it (`view_channel`
    /// and `read_history`): the newest messages, or with `before` the ones that
    /// precede it.
    pub async fn history(
        &self,
        channel_id: &ChannelId,
        user: &User,
        before: Option<MessageId>,
        limit: Option<usize>,
    ) -> Result<ChatPage, ChatFailure> {
        self.communities.history_grant(channel_id, user).await?;
        Ok(self.page(channel_id, before, limit).await?)
    }

    async fn page(
        &self,
        channel_id: &ChannelId,
        before: Option<MessageId>,
        limit: Option<usize>,
    ) -> Result<ChatPage, ChatStoreError> {
        let limit = limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE);
        // One more than asked for tells whether there is anything older.
        let mut newest_first = self.messages.page(channel_id, before, limit + 1).await?;
        let has_more = newest_first.len() > limit;
        newest_first.truncate(limit);
        newest_first.reverse();
        Ok(ChatPage { messages: newest_first, has_more })
    }

    /// Drops what is remembered about a participant. Called when it leaves.
    pub fn forget(&self, session_id: &SessionId) {
        self.limits().remove(session_id);
    }

    fn check_limit(&self, session_id: &SessionId) -> Result<(), ChatError> {
        let now_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let result = self
            .limits()
            .entry(session_id.clone())
            .or_insert_with(SendLimit::standard)
            .record(now_ms);
        if result.is_err() {
            warn!(session = %session_id, "chat rate limit hit");
        }
        result
    }

    fn limits(&self) -> MutexGuard<'_, HashMap<SessionId, SendLimit>> {
        self.limits.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn check_post_limit(&self, user_id: &UserId) -> Result<(), ChatError> {
        let now_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let mut limits = self.post_limits.lock().unwrap_or_else(PoisonError::into_inner);
        limits.entry(user_id.clone()).or_insert_with(SendLimit::standard).record(now_ms)
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
