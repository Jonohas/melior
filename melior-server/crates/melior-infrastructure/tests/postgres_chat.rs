//! The chat history on a real Postgres. Needs a database, so it is ignored by default:
//!
//! ```sh
//! MELIOR_TEST_DATABASE_URL=postgres://user:password@127.0.0.1:5432/db \
//!     cargo test -p melior-infrastructure --test postgres_chat -- --ignored
//! ```

use melior_application::{ChatRepository, CommunityRepository};
use melior_domain::{
    Attachment, AttachmentId, ChannelId, ChannelKind, ChatContent, ChatText, CommunityId,
    CommunityState, GifLink, ImageFormat, MessageId, Name, StoredImage, User, UserId,
};
use melior_infrastructure::postgres::{
    Database, PostgresChatRepository, PostgresCommunityRepository,
};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn database() -> Arc<Database> {
    let url = std::env::var("MELIOR_TEST_DATABASE_URL").expect("MELIOR_TEST_DATABASE_URL");
    Arc::new(Database::new(&url).expect("database url"))
}

/// A channel id no earlier run used, so runs do not disturb each other.
fn fresh_channel() -> ChannelId {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    ChannelId::parse(&format!("t{nanos}")).unwrap()
}

fn text(raw: &str) -> ChatContent {
    ChatText::parse(raw).unwrap().into()
}

/// Stores a community whose voice channel has the id `channel`.
async fn register(database: &Arc<Database>, channel: &ChannelId) -> PostgresCommunityRepository {
    let communities = PostgresCommunityRepository::new(database.clone());
    let owner = User::new(UserId::new("sub-alice"), "alice", None, [User::ROLE_MEMBER.to_owned()]);
    let (mut state, member) =
        CommunityState::create(CommunityId::generate(), Name::parse("Chat").unwrap(), &owner);
    state.channels.retain(|c| c.kind == ChannelKind::Voice);
    state.channels[0].id = channel.clone();
    communities.create(&state, &member).await.unwrap();
    communities
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_DATABASE_URL"]
async fn messages_are_saved_with_an_increasing_id_and_a_time() {
    let database = database();
    let channel = fresh_channel();
    register(&database, &channel).await;
    let chat = PostgresChatRepository::new(database);

    let first = chat.append(&channel, "alice", &text("one")).await.unwrap();
    let second = chat.append(&channel, "bob", &text("two\nlines")).await.unwrap();

    assert!(second.id > first.id);
    assert_eq!((second.author.as_str(), second.content.text_str()), ("bob", "two\nlines"));
    assert!(first.sent_at_ms > 1_600_000_000_000, "time comes from the database clock");
    assert!(second.sent_at_ms >= first.sent_at_ms);
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_DATABASE_URL"]
async fn a_page_is_newest_first_and_before_pages_back() {
    let database = database();
    let channel = fresh_channel();
    register(&database, &channel).await;
    let chat = PostgresChatRepository::new(database);
    for n in 1..=5 {
        chat.append(&channel, "alice", &text(&format!("m{n}"))).await.unwrap();
    }

    let newest = chat.page(&channel, None, 2).await.unwrap();
    let texts = |page: &[melior_domain::ChatMessage]| {
        page.iter().map(|m| m.content.text_str().to_owned()).collect::<Vec<_>>()
    };
    assert_eq!(texts(&newest), ["m5", "m4"]);

    let older = chat.page(&channel, Some(newest[1].id), 10).await.unwrap();
    assert_eq!(texts(&older), ["m3", "m2", "m1"]);
    assert!(chat.page(&channel, Some(MessageId::new(0)), 10).await.unwrap().is_empty());
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_DATABASE_URL"]
async fn channels_keep_their_messages_apart_and_deleting_a_channel_deletes_its_messages() {
    let database = database();
    let (kept, doomed) = (fresh_channel(), fresh_channel());
    register(&database, &kept).await;
    let communities = register(&database, &doomed).await;
    let chat = PostgresChatRepository::new(database);
    chat.append(&kept, "alice", &text("stays")).await.unwrap();
    chat.append(&doomed, "alice", &text("goes")).await.unwrap();

    communities.delete_channel(&doomed).await.unwrap();

    assert!(chat.page(&doomed, None, 10).await.unwrap().is_empty());
    assert_eq!(chat.page(&kept, None, 10).await.unwrap().len(), 1);
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_DATABASE_URL"]
async fn a_message_for_an_unknown_channel_is_refused() {
    let chat = PostgresChatRepository::new(database());

    let result = chat.append(&fresh_channel(), "alice", &text("hello")).await;

    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_DATABASE_URL"]
async fn images_and_gifs_are_saved_with_their_messages() {
    let database = database();
    let channel = fresh_channel();
    register(&database, &channel).await;
    let chat = PostgresChatRepository::new(database);
    let image = |format| StoredImage {
        id: AttachmentId::generate(),
        channel_id: channel.clone(),
        uploader: "alice".into(),
        format,
        size: 42,
    };
    let images = [image(ImageFormat::Webp), image(ImageFormat::Png), image(ImageFormat::Jpeg)];
    for stored in &images {
        chat.save_image(stored).await.unwrap();
    }
    assert_eq!(chat.image(&images[0].id).await.unwrap(), Some(images[0].clone()));
    assert_eq!(chat.image(&AttachmentId::generate()).await.unwrap(), None);
    let mut found = chat.images(&[images[1].id.clone(), AttachmentId::generate()]).await.unwrap();
    found.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(found, [images[1].clone()]);

    // Not in upload order: the message keeps the order it was sent with.
    let attachments: Vec<Attachment> = [2, 0, 1]
        .iter()
        .map(|&i| Attachment::Image { id: images[i].id.clone(), format: images[i].format })
        .collect();
    let with_images = ChatContent::new(ChatText::parse("three").ok(), attachments).unwrap();
    let gif = GifLink::parse("https://static.klipy.com/a.gif", 20, 10).unwrap();
    let with_gif = ChatContent::new(None, vec![Attachment::Gif(gif)]).unwrap();
    let saved_images = chat.append(&channel, "alice", &with_images).await.unwrap();
    let saved_gif = chat.append(&channel, "alice", &with_gif).await.unwrap();

    assert_eq!(saved_images.content, with_images);
    assert_eq!(saved_gif.content, with_gif);
    let page = chat.page(&channel, None, 10).await.unwrap();
    assert_eq!(page.iter().map(|m| m.content.clone()).collect::<Vec<_>>(), [with_gif, with_images]);
    let swept = chat.delete_unattached_images(Duration::ZERO, 1000).await.unwrap();
    assert!(images.iter().all(|image| !swept.contains(&image.id)), "all three are attached");
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_DATABASE_URL"]
async fn only_images_no_message_carries_are_deleted_once_old_enough() {
    let database = database();
    let channel = fresh_channel();
    register(&database, &channel).await;
    let chat = PostgresChatRepository::new(database);
    let image = |format| StoredImage {
        id: AttachmentId::generate(),
        channel_id: channel.clone(),
        uploader: "alice".into(),
        format,
        size: 1,
    };
    let (sent, unsent) = (image(ImageFormat::Png), image(ImageFormat::Gif));
    chat.save_image(&sent).await.unwrap();
    chat.save_image(&unsent).await.unwrap();
    let attachment = Attachment::Image { id: sent.id.clone(), format: ImageFormat::Png };
    chat.append(&channel, "alice", &ChatContent::new(None, vec![attachment]).unwrap())
        .await
        .unwrap();

    let young = chat.delete_unattached_images(Duration::from_secs(3600), 1000).await.unwrap();
    assert!(!young.contains(&unsent.id), "younger than the limit");
    tokio::time::sleep(Duration::from_millis(5)).await;
    let swept = chat.delete_unattached_images(Duration::ZERO, 1000).await.unwrap();

    assert!(swept.contains(&unsent.id));
    assert!(!swept.contains(&sent.id));
    assert_eq!(chat.image(&unsent.id).await.unwrap(), None);
    assert_eq!(chat.image(&sent.id).await.unwrap(), Some(sent.clone()));
    let gone = Attachment::Image { id: unsent.id.clone(), format: ImageFormat::Gif };
    let late = chat.append(&channel, "alice", &ChatContent::new(None, vec![gone]).unwrap()).await;
    assert!(late.is_err(), "a swept image cannot be attached");
}
