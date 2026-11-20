use super::Database;
use async_trait::async_trait;
use melior_application::{ChatRepository, ChatStoreError};
use melior_domain::{ChannelId, ChatMessage, ChatText, MessageId};
use sqlx::Row;
use sqlx::postgres::PgRow;
use std::sync::Arc;

const COLUMNS: &str = "id, channel_id, author, body, sent_at_ms";

/// [`ChatRepository`] on Postgres.
pub struct PostgresChatRepository {
    database: Arc<Database>,
}

impl PostgresChatRepository {
    #[must_use]
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }
}

#[async_trait]
impl ChatRepository for PostgresChatRepository {
    async fn append(
        &self,
        channel_id: &ChannelId,
        author: &str,
        text: &ChatText,
    ) -> Result<ChatMessage, ChatStoreError> {
        let pool = self.database.pool().await.map_err(store_error)?;
        sqlx::query(&format!(
            "INSERT INTO chat_messages (channel_id, author, body) VALUES ($1, $2, $3)
             RETURNING {COLUMNS}"
        ))
        .bind(channel_id.as_str())
        .bind(author)
        .bind(text.as_str())
        .fetch_one(pool)
        .await
        .map_err(store_error)
        .and_then(|row| message_from_row(&row))
    }

    async fn page(
        &self,
        channel_id: &ChannelId,
        before: Option<MessageId>,
        limit: usize,
    ) -> Result<Vec<ChatMessage>, ChatStoreError> {
        let pool = self.database.pool().await.map_err(store_error)?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        sqlx::query(&format!(
            "SELECT {COLUMNS} FROM chat_messages
             WHERE channel_id = $1 AND ($2::bigint IS NULL OR id < $2)
             ORDER BY id DESC
             LIMIT $3"
        ))
        .bind(channel_id.as_str())
        .bind(before.map(MessageId::value))
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(store_error)?
        .iter()
        .map(message_from_row)
        .collect()
    }
}

/// Rebuilds a message from a row. Stored values go through the same validation
/// as user input: a row that does not fit the domain is an error, not a message.
fn message_from_row(row: &PgRow) -> Result<ChatMessage, ChatStoreError> {
    let id: i64 = row.try_get("id").map_err(store_error)?;
    let channel_id: String = row.try_get("channel_id").map_err(store_error)?;
    let author: String = row.try_get("author").map_err(store_error)?;
    let body: String = row.try_get("body").map_err(store_error)?;
    let sent_at_ms: i64 = row.try_get("sent_at_ms").map_err(store_error)?;

    Ok(ChatMessage {
        id: MessageId::new(id),
        channel_id: ChannelId::parse(&channel_id).map_err(store_error)?,
        author,
        text: ChatText::parse(&body).map_err(store_error)?,
        sent_at_ms,
    })
}

fn store_error(error: impl std::fmt::Display) -> ChatStoreError {
    ChatStoreError(error.to_string())
}
