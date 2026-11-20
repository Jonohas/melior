use super::ports::{ChatRepository, ChatStoreError};
use crate::conference::{ConferenceError, ConferenceService, Signal};
use melior_domain::{ChannelId, ChatError, ChatMessage, ChatText, MessageId, SendLimit, SessionId};
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
    limits: Mutex<HashMap<SessionId, SendLimit>>,
    started: Instant,
}

impl ChatService {
    pub fn new(conference: Arc<ConferenceService>, messages: Arc<dyn ChatRepository>) -> Self {
        Self { conference, messages, limits: Mutex::new(HashMap::new()), started: Instant::now() }
    }

    /// Saves what a participant said and tells everyone in its room, the author
    /// included. The author's copy carries `client_id`, which the client chose to
    /// recognise its own message.
    #[tracing::instrument(name = "chat.send", skip_all, fields(session = %session_id))]
    pub async fn send(
        &self,
        session_id: &SessionId,
        client_id: Option<String>,
        raw_text: &str,
    ) -> Result<(), ChatFailure> {
        let (channel_id, author) = self.conference.whereabouts(session_id).await?;
        let text = ChatText::parse(raw_text)?;
        self.check_limit(session_id)?;
        let message = self.messages.append(&channel_id, &author, &text).await?;
        let for_others = Signal::Chat { message: message.clone(), client_id: None };
        let for_sender = Signal::Chat { message, client_id };
        self.conference.tell_room(session_id, for_sender, for_others).await?;
        Ok(())
    }

    /// A page of a room's history: the newest messages, or with `before` the ones
    /// that precede it. The caller has checked that the user may read the room.
    pub async fn history(
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::InMemoryChatRepository;
    use crate::conference::{
        MediaEngine, MediaError, MediaEvents, MediaOffer, MediaSession, SignalSink, SingleInstance,
        StaticIceServers,
    };
    use async_trait::async_trait;
    use melior_domain::{IceCandidate, Sdp, TrackId, User, UserId};
    use std::sync::Weak;

    /// A media plane that does nothing: chat does not touch it.
    struct NoMedia;

    #[async_trait]
    impl MediaEngine for NoMedia {
        async fn create_session(
            &self,
            _session_id: SessionId,
            _events: Weak<dyn MediaEvents>,
        ) -> Result<Arc<dyn MediaSession>, MediaError> {
            Ok(Arc::new(NoMedia))
        }
    }

    #[async_trait]
    impl MediaSession for NoMedia {
        async fn apply_offer(&self, _offer: Sdp) -> Result<Sdp, MediaError> {
            Ok(Sdp::new("answer"))
        }
        async fn create_offer(&self) -> Result<MediaOffer, MediaError> {
            Ok(MediaOffer { sdp: Sdp::new("offer"), carrying: Vec::new() })
        }
        async fn apply_answer(&self, _answer: Sdp) -> Result<(), MediaError> {
            Ok(())
        }
        async fn add_ice_candidate(&self, _candidate: IceCandidate) -> Result<(), MediaError> {
            Ok(())
        }
        async fn subscribe(&self, _track: &TrackId) -> Result<(), MediaError> {
            Ok(())
        }
        async fn unsubscribe(&self, _track: &TrackId) -> Result<(), MediaError> {
            Ok(())
        }
        async fn close(&self) {}
    }

    #[derive(Default)]
    struct Inbox(Mutex<Vec<Signal>>);

    impl Inbox {
        /// The chat messages received so far as `(author, text, client_id)`.
        fn chats(&self) -> Vec<(String, String, Option<String>)> {
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .iter()
                .filter_map(|signal| match signal {
                    Signal::Chat { message, client_id } => {
                        Some((message.author.clone(), message.text.to_string(), client_id.clone()))
                    }
                    _ => None,
                })
                .collect()
        }
    }

    impl SignalSink for Inbox {
        fn send(&self, signal: Signal) {
            self.0.lock().unwrap_or_else(PoisonError::into_inner).push(signal);
        }
    }

    struct Guest {
        id: SessionId,
        inbox: Arc<Inbox>,
    }

    struct Harness {
        conference: Arc<ConferenceService>,
        chat: ChatService,
        store: Arc<InMemoryChatRepository>,
    }

    impl Harness {
        fn new() -> Self {
            let conference = ConferenceService::new(
                Arc::new(NoMedia),
                Arc::new(StaticIceServers::default()),
                Arc::new(SingleInstance),
            );
            let store = Arc::new(InMemoryChatRepository::default());
            let chat = ChatService::new(conference.clone(), store.clone());
            Self { conference, chat, store }
        }

        async fn join(&self, channel: &str, username: &str) -> Guest {
            let user = User::new(
                UserId::new(format!("sub-{username}")),
                username,
                None,
                [User::ROLE_MEMBER.to_owned()],
            );
            let inbox = Arc::new(Inbox::default());
            let id = self
                .conference
                .join(ChannelId::parse(channel).unwrap(), &user, inbox.clone())
                .await
                .unwrap();
            Guest { id, inbox }
        }
    }

    fn channel(name: &str) -> ChannelId {
        ChannelId::parse(name).unwrap()
    }

    #[tokio::test]
    async fn a_message_reaches_the_room_and_only_the_author_gets_its_client_id() {
        let h = Harness::new();
        let alice = h.join("general", "alice").await;
        let bob = h.join("general", "bob").await;

        h.chat.send(&alice.id, Some("c1".into()), "  hello  ").await.unwrap();

        assert_eq!(alice.inbox.chats(), vec![("alice".into(), "hello".into(), Some("c1".into()))]);
        assert_eq!(bob.inbox.chats(), vec![("alice".into(), "hello".into(), None)]);
    }

    #[tokio::test]
    async fn a_message_is_saved_with_its_author_and_room() {
        let h = Harness::new();
        let alice = h.join("general", "alice").await;

        h.chat.send(&alice.id, None, "hello").await.unwrap();

        let saved = h.store.page(&channel("general"), None, 10).await.unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!((saved[0].author.as_str(), saved[0].text.as_str()), ("alice", "hello"));
    }

    #[tokio::test]
    async fn another_room_does_not_hear_it() {
        let h = Harness::new();
        let alice = h.join("general", "alice").await;
        let carol = h.join("other", "carol").await;

        h.chat.send(&alice.id, None, "hello").await.unwrap();

        assert!(carol.inbox.chats().is_empty());
        assert!(h.store.page(&channel("other"), None, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_message_that_breaks_a_rule_is_neither_saved_nor_sent() {
        let h = Harness::new();
        let alice = h.join("general", "alice").await;
        let bob = h.join("general", "bob").await;

        let result = h.chat.send(&alice.id, None, "   ").await;

        assert!(matches!(result, Err(ChatFailure::Rejected(ChatError::Empty))));
        assert!(bob.inbox.chats().is_empty());
        assert!(h.store.page(&channel("general"), None, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_participant_cannot_flood_the_room() {
        let h = Harness::new();
        let alice = h.join("general", "alice").await;
        let bob = h.join("general", "bob").await;

        for _ in 0..5 {
            h.chat.send(&alice.id, None, "spam").await.unwrap();
        }
        let sixth = h.chat.send(&alice.id, None, "spam").await;

        assert!(matches!(sixth, Err(ChatFailure::Rejected(ChatError::RateLimited))));
        assert_eq!(bob.inbox.chats().len(), 5);
        // Another participant has a limit of its own.
        h.chat.send(&bob.id, None, "hi").await.unwrap();
    }

    #[tokio::test]
    async fn forgetting_a_participant_clears_its_send_limit() {
        let h = Harness::new();
        let alice = h.join("general", "alice").await;
        for _ in 0..5 {
            h.chat.send(&alice.id, None, "spam").await.unwrap();
        }

        h.chat.forget(&alice.id);

        assert!(h.chat.send(&alice.id, None, "again").await.is_ok());
    }

    #[tokio::test]
    async fn an_unknown_session_cannot_chat() {
        let h = Harness::new();

        let result = h.chat.send(&SessionId::generate(), None, "hello").await;

        assert!(matches!(result, Err(ChatFailure::Conference(ConferenceError::UnknownSession))));
    }

    #[tokio::test]
    async fn history_pages_back_from_the_newest_message_oldest_first() {
        let h = Harness::new();
        let room = channel("general");
        for n in 1..=5 {
            h.store
                .append(&room, "alice", &ChatText::parse(&format!("m{n}")).unwrap())
                .await
                .unwrap();
        }

        let newest = h.chat.history(&room, None, Some(2)).await.unwrap();
        let texts =
            |page: &ChatPage| page.messages.iter().map(|m| m.text.to_string()).collect::<Vec<_>>();
        assert_eq!(texts(&newest), ["m4", "m5"]);
        assert!(newest.has_more);

        let before = newest.messages[0].id;
        let older = h.chat.history(&room, Some(before), Some(10)).await.unwrap();
        assert_eq!(texts(&older), ["m1", "m2", "m3"]);
        assert!(!older.has_more);
    }

    #[tokio::test]
    async fn history_of_an_untouched_room_is_empty() {
        let h = Harness::new();

        let page = h.chat.history(&channel("quiet"), None, None).await.unwrap();

        assert_eq!(page, ChatPage { messages: Vec::new(), has_more: false });
    }

    #[tokio::test]
    async fn a_page_size_is_kept_within_bounds() {
        let h = Harness::new();
        let room = channel("general");
        for n in 0..3 {
            h.store
                .append(&room, "alice", &ChatText::parse(&format!("m{n}")).unwrap())
                .await
                .unwrap();
        }

        assert_eq!(h.chat.history(&room, None, Some(0)).await.unwrap().messages.len(), 1);
        assert_eq!(h.chat.history(&room, None, Some(100_000)).await.unwrap().messages.len(), 3);
    }
}
