use super::limits::KeyedLimits;
use super::ports::{ChatRepository, ChatStoreError, ImageStore, ImageStoreError};
use crate::communities::{CommunitiesError, CommunityEvent, CommunityService};
use crate::conference::{ConferenceError, ConferenceService, Signal};
use melior_domain::{
    Attachment, AttachmentRequest, ChannelId, ChatContent, ChatError, ChatMessage, ChatText,
    MessageId, Permissions, SendLimit, SessionId, User, UserId,
};
use std::sync::Arc;
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

    /// The image storage is off or cannot be reached.
    #[error(transparent)]
    Images(#[from] ImageStoreError),

    /// The channel or image is unknown, of the wrong kind, or closed to the user.
    #[error(transparent)]
    Community(#[from] CommunitiesError),
}

/// What a client asks to say: raw text, checked by the service, and an
/// attachment already parsed at the edge.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChatDraft {
    pub text: String,
    pub attachment: Option<AttachmentRequest>,
}

impl From<&str> for ChatDraft {
    fn from(text: &str) -> Self {
        Self { text: text.to_owned(), attachment: None }
    }
}

/// One page of a room's history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatPage {
    /// Oldest first.
    pub messages: Vec<ChatMessage>,
    /// Whether older messages exist than the oldest one here.
    pub has_more: bool,
}

/// The chat use cases: saving what a participant says and telling the room,
/// reading what was said before, and the images messages carry.
pub struct ChatService {
    conference: Arc<ConferenceService>,
    pub(super) messages: Arc<dyn ChatRepository>,
    pub(super) images: Arc<dyn ImageStore>,
    pub(super) communities: Arc<CommunityService>,
    limits: KeyedLimits<SessionId>,
    /// The limits of text channel posts, per user, since a user has no session there.
    post_limits: KeyedLimits<UserId>,
    /// The limits of image uploads, per user.
    pub(super) upload_limits: KeyedLimits<UserId>,
}

impl ChatService {
    pub fn new(
        conference: Arc<ConferenceService>,
        messages: Arc<dyn ChatRepository>,
        images: Arc<dyn ImageStore>,
        communities: Arc<CommunityService>,
    ) -> Self {
        Self {
            conference,
            messages,
            images,
            communities,
            limits: KeyedLimits::new(SendLimit::standard),
            post_limits: KeyedLimits::new(SendLimit::standard),
            upload_limits: KeyedLimits::new(SendLimit::uploads),
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
        draft: ChatDraft,
        permissions: Permissions,
    ) -> Result<(), ChatFailure> {
        let (channel_id, author) = self.conference.whereabouts(session_id).await?;
        ChatError::require_send(permissions)?;
        let content = self.compose(&channel_id, &author, draft).await?;
        if let Err(error) = self.limits.record(session_id) {
            warn!(session = %session_id, "chat rate limit hit");
            return Err(error.into());
        }
        let message = self.messages.append(&channel_id, &author, &content).await?;
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
        draft: ChatDraft,
    ) -> Result<ChatMessage, ChatFailure> {
        let grant = self.communities.text_grant(channel_id, user).await?;
        let content = self.compose(channel_id, &user.username, draft).await?;
        self.post_limits.record(&user.id)?;
        let message = self.messages.append(channel_id, &user.username, &content).await?;
        let event =
            CommunityEvent::Message { channel_id: channel_id.clone(), message: message.clone() };
        if let Err(error) = self.communities.events().publish(&grant.community_id, event).await {
            warn!(%error, channel = %channel_id, "chat message not published");
        }
        Ok(message)
    }

    /// Checks a draft into what a message by `author` in `channel_id` says. An
    /// image is looked up: only its uploader may attach it, in its own channel.
    async fn compose(
        &self,
        channel_id: &ChannelId,
        author: &str,
        draft: ChatDraft,
    ) -> Result<ChatContent, ChatFailure> {
        let text = ChatText::parse_optional(&draft.text)?;
        let attachment = match draft.attachment {
            None => None,
            Some(AttachmentRequest::Gif(gif)) => Some(Attachment::Gif(gif)),
            Some(AttachmentRequest::Image(id)) => {
                let image = self.messages.image(&id).await?.ok_or(ChatError::UnknownImage)?;
                Some(image.attach(channel_id, author)?)
            }
        };
        Ok(ChatContent::new(text, attachment)?)
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
        self.limits.forget(session_id);
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
