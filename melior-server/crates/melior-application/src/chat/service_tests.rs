//! The chat use cases against fake ports.

use super::*;
use crate::chat::ImageStoreError;
use crate::chat::{InMemoryChatRepository, InMemoryImageStore, UnconfiguredImageStore};
use crate::communities::{
    InMemoryCommunityEvents, InMemoryCommunityRepository, InMemoryVoiceRoster,
};
use crate::conference::{
    Admission, MediaEngine, MediaError, MediaEvents, MediaOffer, MediaSession, NoPresence,
    SignalSink, SingleInstance, StaticIceServers,
};
use async_trait::async_trait;
use melior_domain::{Attachment, AttachmentRequest};
use melior_domain::{
    ChannelKind, CommunityId, IceCandidate, InviteSettings, MeshPolicy, RoomPlan, Sdp, SyncGroup,
    TrackId,
};
use std::sync::{Mutex, PoisonError, Weak};
use std::time::Duration;

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
    async fn subscribe(&self, _track: &TrackId, _group: &SyncGroup) -> Result<(), MediaError> {
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
                Signal::Chat { message, client_id } => Some((
                    message.author.clone(),
                    message.content.text_str().to_owned(),
                    client_id.clone(),
                )),
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
    images: Arc<InMemoryImageStore>,
    communities: Arc<CommunityService>,
}

/// A community of alice (owner) and bob, with its seeded text and voice channels.
struct Town {
    id: CommunityId,
    text: ChannelId,
    voice: ChannelId,
}

fn user(username: &str) -> User {
    User::new(
        UserId::new(format!("sub-{username}")),
        username,
        None,
        [User::ROLE_MEMBER.to_owned()],
    )
}

impl Harness {
    fn new() -> Self {
        let conference = ConferenceService::new(
            Arc::new(NoMedia),
            Arc::new(StaticIceServers::default()),
            Arc::new(SingleInstance),
            MeshPolicy::new(0),
            Arc::new(NoPresence),
        );
        let store = Arc::new(InMemoryChatRepository::default());
        let communities = Arc::new(CommunityService::new(
            Arc::new(InMemoryCommunityRepository::default()),
            Arc::new(InMemoryCommunityEvents::default()),
            Arc::new(InMemoryVoiceRoster::default()),
            MeshPolicy::OFF,
        ));
        let images = Arc::new(InMemoryImageStore::default());
        let chat = ChatService::new(
            conference.clone(),
            store.clone(),
            images.clone(),
            communities.clone(),
        );
        Self { conference, chat, store, images, communities }
    }

    async fn join(&self, channel: &str, username: &str) -> Guest {
        let inbox = Arc::new(Inbox::default());
        let admission = Admission::unlisted(channel_id(channel), RoomPlan::default());
        let id = self.conference.join(admission, &user(username), inbox.clone()).await.unwrap();
        Guest { id, inbox }
    }

    async fn town(&self) -> Town {
        let view = self.communities.create(&user("alice"), "Town").await.unwrap();
        let id = view.community.id.clone();
        let invite = self
            .communities
            .create_invite(&id, &user("alice"), InviteSettings::default())
            .await
            .unwrap();
        self.communities.redeem_invite(&invite.code, &user("bob")).await.unwrap();
        let of = |kind| {
            view.channels.iter().find(|c| c.channel.kind == kind).unwrap().channel.id.clone()
        };
        Town { text: of(ChannelKind::Text), voice: of(ChannelKind::Voice), id }
    }
}

fn channel_id(name: &str) -> ChannelId {
    ChannelId::parse(name).unwrap()
}

#[tokio::test]
async fn a_message_reaches_the_room_and_only_the_author_gets_its_client_id() {
    let h = Harness::new();
    let alice = h.join("general", "alice").await;
    let bob = h.join("general", "bob").await;

    h.chat.send(&alice.id, Some("c1".into()), "  hello  ".into(), Permissions::ALL).await.unwrap();

    assert_eq!(alice.inbox.chats(), vec![("alice".into(), "hello".into(), Some("c1".into()))]);
    assert_eq!(bob.inbox.chats(), vec![("alice".into(), "hello".into(), None)]);
}

#[tokio::test]
async fn a_message_is_saved_with_its_author_and_room() {
    let h = Harness::new();
    let alice = h.join("general", "alice").await;

    h.chat.send(&alice.id, None, "hello".into(), Permissions::ALL).await.unwrap();

    let saved = h.store.page(&channel_id("general"), None, 10).await.unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!((saved[0].author.as_str(), saved[0].content.text_str()), ("alice", "hello"));
}

#[tokio::test]
async fn another_room_does_not_hear_it() {
    let h = Harness::new();
    let alice = h.join("general", "alice").await;
    let carol = h.join("other", "carol").await;

    h.chat.send(&alice.id, None, "hello".into(), Permissions::ALL).await.unwrap();

    assert!(carol.inbox.chats().is_empty());
    assert!(h.store.page(&channel_id("other"), None, 10).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_message_that_breaks_a_rule_is_neither_saved_nor_sent() {
    let h = Harness::new();
    let alice = h.join("general", "alice").await;
    let bob = h.join("general", "bob").await;

    let result = h.chat.send(&alice.id, None, "   ".into(), Permissions::ALL).await;

    assert!(matches!(result, Err(ChatFailure::Rejected(ChatError::Empty))));
    assert!(bob.inbox.chats().is_empty());
    assert!(h.store.page(&channel_id("general"), None, 10).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_participant_cannot_flood_the_room() {
    let h = Harness::new();
    let alice = h.join("general", "alice").await;
    let bob = h.join("general", "bob").await;

    for _ in 0..5 {
        h.chat.send(&alice.id, None, "spam".into(), Permissions::ALL).await.unwrap();
    }
    let sixth = h.chat.send(&alice.id, None, "spam".into(), Permissions::ALL).await;

    assert!(matches!(sixth, Err(ChatFailure::Rejected(ChatError::RateLimited))));
    assert_eq!(bob.inbox.chats().len(), 5);
    // Another participant has a limit of its own.
    h.chat.send(&bob.id, None, "hi".into(), Permissions::ALL).await.unwrap();
}

#[tokio::test]
async fn forgetting_a_participant_clears_its_send_limit() {
    let h = Harness::new();
    let alice = h.join("general", "alice").await;
    for _ in 0..5 {
        h.chat.send(&alice.id, None, "spam".into(), Permissions::ALL).await.unwrap();
    }

    h.chat.forget(&alice.id);

    assert!(h.chat.send(&alice.id, None, "again".into(), Permissions::ALL).await.is_ok());
}

#[tokio::test]
async fn an_unknown_session_cannot_chat() {
    let h = Harness::new();

    let result = h.chat.send(&SessionId::generate(), None, "hello".into(), Permissions::ALL).await;

    assert!(matches!(result, Err(ChatFailure::Conference(ConferenceError::UnknownSession))));
}

#[tokio::test]
async fn history_pages_back_from_the_newest_message_oldest_first() {
    let h = Harness::new();
    let room = channel_id("general");
    for n in 1..=5 {
        h.store
            .append(&room, "alice", &ChatText::parse(&format!("m{n}")).unwrap().into())
            .await
            .unwrap();
    }

    let newest = h.chat.page(&room, None, Some(2)).await.unwrap();
    let texts = |page: &ChatPage| {
        page.messages.iter().map(|m| m.content.text_str().to_owned()).collect::<Vec<_>>()
    };
    assert_eq!(texts(&newest), ["m4", "m5"]);
    assert!(newest.has_more);

    let before = newest.messages[0].id;
    let older = h.chat.page(&room, Some(before), Some(10)).await.unwrap();
    assert_eq!(texts(&older), ["m1", "m2", "m3"]);
    assert!(!older.has_more);
}

#[tokio::test]
async fn history_of_an_untouched_room_is_empty() {
    let h = Harness::new();

    let page = h.chat.page(&channel_id("quiet"), None, None).await.unwrap();

    assert_eq!(page, ChatPage { messages: Vec::new(), has_more: false });
}

#[tokio::test]
async fn a_page_size_is_kept_within_bounds() {
    let h = Harness::new();
    let room = channel_id("general");
    for n in 0..3 {
        h.store
            .append(&room, "alice", &ChatText::parse(&format!("m{n}")).unwrap().into())
            .await
            .unwrap();
    }

    assert_eq!(h.chat.page(&room, None, Some(0)).await.unwrap().messages.len(), 1);
    assert_eq!(h.chat.page(&room, None, Some(100_000)).await.unwrap().messages.len(), 3);
}

#[tokio::test]
async fn a_participant_without_send_messages_is_refused_and_nothing_is_saved() {
    let h = Harness::new();
    let alice = h.join("general", "alice").await;

    let result = h.chat.send(&alice.id, None, "hi".into(), Permissions::VIEW_CHANNEL).await;

    assert!(matches!(result, Err(ChatFailure::Rejected(ChatError::NotAllowed))));
    assert!(h.store.page(&channel_id("general"), None, 10).await.unwrap().is_empty());
}

#[tokio::test]
async fn posting_to_a_text_channel_saves_and_publishes_the_message() {
    let h = Harness::new();
    let town = h.town().await;
    let mut feed = h.communities.events().subscribe(&town.id);

    let message = h.chat.post(&town.text, &user("bob"), "  hello  ".into()).await.unwrap();

    assert_eq!((message.author.as_str(), message.content.text_str()), ("bob", "hello"));
    assert_eq!(
        feed.recv().await.unwrap(),
        CommunityEvent::Message { channel_id: town.text.clone(), message }
    );
    let page = h.chat.history(&town.text, &user("alice"), None, None).await.unwrap();
    assert_eq!(page.messages.len(), 1);
}

#[tokio::test]
async fn posting_to_a_voice_channel_is_the_wrong_kind() {
    let h = Harness::new();
    let town = h.town().await;

    let result = h.chat.post(&town.voice, &user("bob"), "hello".into()).await;

    assert!(matches!(result, Err(ChatFailure::Community(CommunitiesError::WrongKind))));
}

#[tokio::test]
async fn strangers_and_unknown_channels_get_no_text_channel() {
    let h = Harness::new();
    let town = h.town().await;

    let stranger = h.chat.post(&town.text, &user("carol"), "hi".into()).await;
    assert!(matches!(stranger, Err(ChatFailure::Community(CommunitiesError::Forbidden))));
    let history = h.chat.history(&town.text, &user("carol"), None, None).await;
    assert!(matches!(history, Err(ChatFailure::Community(CommunitiesError::Forbidden))));
    let unknown = h.chat.post(&channel_id("nowhere"), &user("bob"), "hi".into()).await;
    assert!(matches!(unknown, Err(ChatFailure::Community(CommunitiesError::NotFound))));
}

#[tokio::test]
async fn posts_are_rate_limited_per_user() {
    let h = Harness::new();
    let town = h.town().await;
    for _ in 0..5 {
        h.chat.post(&town.text, &user("bob"), "spam".into()).await.unwrap();
    }

    let sixth = h.chat.post(&town.text, &user("bob"), "spam".into()).await;

    assert!(matches!(sixth, Err(ChatFailure::Rejected(ChatError::RateLimited))));
    assert!(h.chat.post(&town.text, &user("alice"), "hi".into()).await.is_ok());
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n-png-body";

fn with_image(text: &str, id: &melior_domain::AttachmentId) -> ChatDraft {
    ChatDraft { text: text.into(), attachments: vec![AttachmentRequest::Image(id.clone())] }
}

#[tokio::test]
async fn an_uploaded_image_is_sent_with_a_message_and_shown_to_readers() {
    let h = Harness::new();
    let town = h.town().await;

    let image = h.chat.upload_image(&town.text, &user("bob"), PNG.to_vec()).await.unwrap();
    let message = h.chat.post(&town.text, &user("bob"), with_image("", &image.id)).await.unwrap();

    assert_eq!(message.content.text(), None);
    assert_eq!(
        message.content.attachments(),
        [Attachment::Image { id: image.id.clone(), format: melior_domain::ImageFormat::Png }]
    );
    let file = h.chat.image(&image.id, &user("alice")).await.unwrap();
    assert_eq!((file.image, file.bytes.as_slice()), (image, PNG));
}

#[tokio::test]
async fn an_image_can_be_uploaded_for_a_voice_channel_and_sent_in_its_room() {
    let h = Harness::new();
    let town = h.town().await;
    let image = h.chat.upload_image(&town.voice, &user("alice"), PNG.to_vec()).await.unwrap();
    let alice = h.join(town.voice.as_str(), "alice").await;

    h.chat.send(&alice.id, None, with_image("look", &image.id), Permissions::ALL).await.unwrap();

    assert_eq!(alice.inbox.chats(), vec![("alice".into(), "look".into(), None)]);
}

#[tokio::test]
async fn anything_but_an_image_is_refused_and_not_stored() {
    let h = Harness::new();
    let town = h.town().await;

    let svg = h.chat.upload_image(&town.text, &user("bob"), b"<svg/>".to_vec()).await;

    assert!(matches!(svg, Err(ChatFailure::Rejected(ChatError::UnsupportedImage))));
}

#[tokio::test]
async fn strangers_cannot_upload_or_see_images() {
    let h = Harness::new();
    let town = h.town().await;
    let image = h.chat.upload_image(&town.text, &user("bob"), PNG.to_vec()).await.unwrap();

    let upload = h.chat.upload_image(&town.text, &user("carol"), PNG.to_vec()).await;
    assert!(matches!(upload, Err(ChatFailure::Community(CommunitiesError::Forbidden))));
    let view = h.chat.image(&image.id, &user("carol")).await;
    assert!(matches!(view, Err(ChatFailure::Community(CommunitiesError::Forbidden))));
    let unknown = h.chat.image(&melior_domain::AttachmentId::generate(), &user("bob")).await;
    assert!(matches!(unknown, Err(ChatFailure::Community(CommunitiesError::NotFound))));
}

#[tokio::test]
async fn an_image_of_someone_else_cannot_be_attached() {
    let h = Harness::new();
    let town = h.town().await;
    let image = h.chat.upload_image(&town.text, &user("bob"), PNG.to_vec()).await.unwrap();

    let theft = h.chat.post(&town.text, &user("alice"), with_image("mine", &image.id)).await;

    assert!(matches!(theft, Err(ChatFailure::Rejected(ChatError::UnknownImage))));
}

#[tokio::test]
async fn uploads_are_rate_limited_per_user() {
    let h = Harness::new();
    let town = h.town().await;
    for _ in 0..30 {
        h.chat.upload_image(&town.text, &user("bob"), PNG.to_vec()).await.unwrap();
    }

    let over = h.chat.upload_image(&town.text, &user("bob"), PNG.to_vec()).await;

    assert!(matches!(over, Err(ChatFailure::Rejected(ChatError::RateLimited))));
}

#[tokio::test]
async fn without_image_storage_uploads_fail_as_unconfigured() {
    let mut h = Harness::new();
    h.chat = ChatService::new(
        h.conference.clone(),
        h.store.clone(),
        Arc::new(UnconfiguredImageStore),
        h.communities.clone(),
    );
    let town = h.town().await;

    let upload = h.chat.upload_image(&town.text, &user("bob"), PNG.to_vec()).await;

    assert!(matches!(upload, Err(ChatFailure::Images(ImageStoreError::Unconfigured))));
}

#[tokio::test]
async fn a_gif_is_a_message_of_its_own() {
    let h = Harness::new();
    let town = h.town().await;
    let gif = melior_domain::GifLink::parse("https://static.klipy.com/a.gif", 200, 100).unwrap();
    let draft =
        ChatDraft { text: " ".into(), attachments: vec![AttachmentRequest::Gif(gif.clone())] };

    let message = h.chat.post(&town.text, &user("bob"), draft).await.unwrap();

    assert_eq!(message.content.attachments(), [Attachment::Gif(gif)]);
}

#[tokio::test]
async fn a_sweep_removes_unattached_images_and_keeps_attached_ones() {
    let h = Harness::new();
    let town = h.town().await;
    let sent = h.chat.upload_image(&town.text, &user("bob"), PNG.to_vec()).await.unwrap();
    let unsent = h.chat.upload_image(&town.text, &user("bob"), PNG.to_vec()).await.unwrap();
    h.chat.post(&town.text, &user("bob"), with_image("", &sent.id)).await.unwrap();

    let removed = h.chat.sweep_unattached_images(Duration::ZERO).await.unwrap();

    assert_eq!(removed, 1);
    assert!(h.chat.image(&sent.id, &user("alice")).await.is_ok());
    let gone = h.chat.image(&unsent.id, &user("alice")).await;
    assert!(matches!(gone, Err(ChatFailure::Community(CommunitiesError::NotFound))));
    assert_eq!(h.images.get(&unsent.id).await.unwrap(), None, "the bytes go too");
}

#[tokio::test]
async fn a_sweep_leaves_images_that_are_younger_than_the_limit() {
    let h = Harness::new();
    let town = h.town().await;
    let image = h.chat.upload_image(&town.text, &user("bob"), PNG.to_vec()).await.unwrap();

    let removed = h.chat.sweep_unattached_images(Duration::from_secs(3600)).await.unwrap();

    assert_eq!(removed, 0);
    assert!(h.chat.image(&image.id, &user("bob")).await.is_ok());
}

#[tokio::test]
async fn a_swept_image_can_no_longer_be_attached() {
    let h = Harness::new();
    let town = h.town().await;
    let image = h.chat.upload_image(&town.text, &user("bob"), PNG.to_vec()).await.unwrap();
    h.chat.sweep_unattached_images(Duration::ZERO).await.unwrap();

    let late = h.chat.post(&town.text, &user("bob"), with_image("", &image.id)).await;

    assert!(matches!(late, Err(ChatFailure::Rejected(ChatError::UnknownImage))));
}

#[tokio::test]
async fn several_images_go_in_one_message_in_their_order() {
    let h = Harness::new();
    let town = h.town().await;
    let mut ids = Vec::new();
    for _ in 0..3 {
        ids.push(h.chat.upload_image(&town.text, &user("bob"), PNG.to_vec()).await.unwrap().id);
    }
    let draft = ChatDraft {
        text: "three".into(),
        attachments: ids.iter().rev().cloned().map(AttachmentRequest::Image).collect(),
    };

    let message = h.chat.post(&town.text, &user("bob"), draft).await.unwrap();

    let sent: Vec<_> = message.content.image_ids().into_iter().cloned().collect();
    assert_eq!(sent, ids.into_iter().rev().collect::<Vec<_>>());
    assert_eq!(h.chat.sweep_unattached_images(Duration::ZERO).await.unwrap(), 0, "all attached");
}

#[tokio::test]
async fn one_unknown_image_refuses_the_whole_message() {
    let h = Harness::new();
    let town = h.town().await;
    let known = h.chat.upload_image(&town.text, &user("bob"), PNG.to_vec()).await.unwrap();
    let draft = ChatDraft {
        text: String::new(),
        attachments: vec![
            AttachmentRequest::Image(known.id.clone()),
            AttachmentRequest::Image(melior_domain::AttachmentId::generate()),
        ],
    };

    let result = h.chat.post(&town.text, &user("bob"), draft).await;

    assert!(matches!(result, Err(ChatFailure::Rejected(ChatError::UnknownImage))));
    assert_eq!(h.chat.sweep_unattached_images(Duration::ZERO).await.unwrap(), 1, "not attached");
}
