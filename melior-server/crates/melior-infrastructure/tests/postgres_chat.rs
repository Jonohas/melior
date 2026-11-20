//! The chat history on a real Postgres. Needs a database, so it is ignored by default:
//!
//! ```sh
//! MELIOR_TEST_DATABASE_URL=postgres://user:password@127.0.0.1:5432/db \
//!     cargo test -p melior-infrastructure --test postgres_chat -- --ignored
//! ```

use melior_application::{ChatRepository, RoomRepository};
use melior_domain::{ChannelId, ChatText, MessageId, RoomAccess, User, UserId};
use melior_infrastructure::postgres::{Database, PostgresChatRepository, PostgresRoomRepository};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn database() -> Arc<Database> {
    let url = std::env::var("MELIOR_TEST_DATABASE_URL").expect("MELIOR_TEST_DATABASE_URL");
    Arc::new(Database::new(&url).expect("database url"))
}

/// A room name no earlier run used, so runs do not disturb each other.
fn fresh_channel() -> ChannelId {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    ChannelId::parse(&format!("t{nanos}")).unwrap()
}

fn text(raw: &str) -> ChatText {
    ChatText::parse(raw).unwrap()
}

async fn register(database: &Arc<Database>, channel: &ChannelId) -> PostgresRoomRepository {
    let rooms = PostgresRoomRepository::new(database.clone());
    let owner = User::new(UserId::new("sub-alice"), "alice", None, [User::ROLE_MEMBER.to_owned()]);
    rooms.insert_if_absent(&RoomAccess::open(channel.clone(), &owner)).await.unwrap();
    rooms
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
    assert_eq!((second.author.as_str(), second.text.as_str()), ("bob", "two\nlines"));
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
        page.iter().map(|m| m.text.to_string()).collect::<Vec<_>>()
    };
    assert_eq!(texts(&newest), ["m5", "m4"]);

    let older = chat.page(&channel, Some(newest[1].id), 10).await.unwrap();
    assert_eq!(texts(&older), ["m3", "m2", "m1"]);
    assert!(chat.page(&channel, Some(MessageId::new(0)), 10).await.unwrap().is_empty());
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_DATABASE_URL"]
async fn rooms_keep_their_messages_apart_and_deleting_a_room_deletes_its_messages() {
    let database = database();
    let (kept, doomed) = (fresh_channel(), fresh_channel());
    register(&database, &kept).await;
    let rooms = register(&database, &doomed).await;
    let chat = PostgresChatRepository::new(database);
    chat.append(&kept, "alice", &text("stays")).await.unwrap();
    chat.append(&doomed, "alice", &text("goes")).await.unwrap();

    rooms.delete(&doomed).await.unwrap();

    assert!(chat.page(&doomed, None, 10).await.unwrap().is_empty());
    assert_eq!(chat.page(&kept, None, 10).await.unwrap().len(), 1);
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_DATABASE_URL"]
async fn a_message_for_an_unregistered_room_is_refused() {
    let chat = PostgresChatRepository::new(database());

    let result = chat.append(&fresh_channel(), "alice", &text("hello")).await;

    assert!(result.is_err());
}
