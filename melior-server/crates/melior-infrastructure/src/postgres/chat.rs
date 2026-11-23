use super::Database;
use async_trait::async_trait;
use melior_application::{ChatRepository, ChatStoreError};
use melior_domain::{
    Attachment, AttachmentId, ChannelId, ChatContent, ChatMessage, ChatText, GifLink, ImageFormat,
    MessageId, StoredImage,
};
use sqlx::Row;
use sqlx::postgres::PgRow;
use std::sync::Arc;
use std::time::Duration;

/// A message's columns, with its image's media type from `chat_images` (`i`).
const COLUMNS: &str = "m.id, m.channel_id, m.author, m.body, m.sent_at_ms, m.image_id, \
                       i.content_type AS image_type, m.gif_url, m.gif_width, m.gif_height";

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
        content: &ChatContent,
    ) -> Result<ChatMessage, ChatStoreError> {
        let pool = self.database.pool().await.map_err(store_error)?;
        let (image_id, gif) = attachment_columns(content.attachment());
        // The image's foreign key locks its record until this commits, so a
        // sweep that runs at the same time sees it attached and leaves it.
        sqlx::query(&format!(
            "WITH m AS (
                INSERT INTO chat_messages (channel_id, author, body, image_id, gif_url, gif_width, gif_height)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                RETURNING *
             ), attached AS (
                UPDATE chat_images
                SET attached_at_ms = (extract(epoch FROM clock_timestamp()) * 1000)::bigint
                WHERE id = $4 AND attached_at_ms IS NULL
             )
             SELECT {COLUMNS} FROM m LEFT JOIN chat_images i ON i.id = m.image_id"
        ))
        .bind(channel_id.as_str())
        .bind(author)
        .bind(content.text_str())
        .bind(image_id)
        .bind(gif.map(GifLink::url))
        .bind(gif.map(|gif| i32::from(gif.width())))
        .bind(gif.map(|gif| i32::from(gif.height())))
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
            "SELECT {COLUMNS} FROM chat_messages m LEFT JOIN chat_images i ON i.id = m.image_id
             WHERE m.channel_id = $1 AND ($2::bigint IS NULL OR m.id < $2)
             ORDER BY m.id DESC
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

    async fn save_image(&self, image: &StoredImage) -> Result<(), ChatStoreError> {
        let pool = self.database.pool().await.map_err(store_error)?;
        sqlx::query(
            "INSERT INTO chat_images (id, channel_id, uploader, content_type, size)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(image.id.as_str())
        .bind(image.channel_id.as_str())
        .bind(&image.uploader)
        .bind(image.format.content_type())
        .bind(i64::try_from(image.size).unwrap_or(i64::MAX))
        .execute(pool)
        .await
        .map_err(store_error)?;
        Ok(())
    }

    async fn delete_unattached_images(
        &self,
        older_than: Duration,
        limit: usize,
    ) -> Result<Vec<AttachmentId>, ChatStoreError> {
        let pool = self.database.pool().await.map_err(store_error)?;
        let older_than_ms = i64::try_from(older_than.as_millis()).unwrap_or(i64::MAX);
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        // SKIP LOCKED: instances sweeping at once take different images, and an
        // image a message is attaching right now is not touched.
        let ids: Vec<String> = sqlx::query_scalar(
            "DELETE FROM chat_images
             WHERE id IN (
                SELECT id FROM chat_images
                WHERE attached_at_ms IS NULL
                  AND uploaded_at_ms < (extract(epoch FROM clock_timestamp()) * 1000)::bigint - $1
                ORDER BY uploaded_at_ms
                LIMIT $2
                FOR UPDATE SKIP LOCKED
             )
             AND attached_at_ms IS NULL
             RETURNING id",
        )
        .bind(older_than_ms)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(store_error)?;
        ids.iter().map(|id| attachment_id(id)).collect()
    }

    async fn image(&self, id: &AttachmentId) -> Result<Option<StoredImage>, ChatStoreError> {
        let pool = self.database.pool().await.map_err(store_error)?;
        sqlx::query(
            "SELECT id, channel_id, uploader, content_type, size FROM chat_images WHERE id = $1",
        )
        .bind(id.as_str())
        .fetch_optional(pool)
        .await
        .map_err(store_error)?
        .map(|row| image_from_row(&row))
        .transpose()
    }
}

/// The `image_id` and GIF columns of an attachment.
fn attachment_columns(attachment: Option<&Attachment>) -> (Option<&str>, Option<&GifLink>) {
    match attachment {
        Some(Attachment::Image { id, .. }) => (Some(id.as_str()), None),
        Some(Attachment::Gif(gif)) => (None, Some(gif)),
        None => (None, None),
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

    let text = ChatText::parse_optional(&body).map_err(store_error)?;
    let content = ChatContent::new(text, attachment_from_row(row)?).map_err(store_error)?;
    Ok(ChatMessage {
        id: MessageId::new(id),
        channel_id: ChannelId::parse(&channel_id).map_err(store_error)?,
        author,
        content,
        sent_at_ms,
    })
}

fn attachment_from_row(row: &PgRow) -> Result<Option<Attachment>, ChatStoreError> {
    let image_id: Option<String> = row.try_get("image_id").map_err(store_error)?;
    if let Some(image_id) = image_id {
        let image_type: Option<String> = row.try_get("image_type").map_err(store_error)?;
        let id = attachment_id(&image_id)?;
        let format = image_format(image_type.as_deref().unwrap_or_default())?;
        return Ok(Some(Attachment::Image { id, format }));
    }
    let gif_url: Option<String> = row.try_get("gif_url").map_err(store_error)?;
    let Some(gif_url) = gif_url else {
        return Ok(None);
    };
    let width: Option<i32> = row.try_get("gif_width").map_err(store_error)?;
    let height: Option<i32> = row.try_get("gif_height").map_err(store_error)?;
    let side = |value: Option<i32>| value.and_then(|v| u32::try_from(v).ok()).unwrap_or(0);
    let gif = GifLink::parse(&gif_url, side(width), side(height)).map_err(store_error)?;
    Ok(Some(Attachment::Gif(gif)))
}

fn image_from_row(row: &PgRow) -> Result<StoredImage, ChatStoreError> {
    let id: String = row.try_get("id").map_err(store_error)?;
    let channel_id: String = row.try_get("channel_id").map_err(store_error)?;
    let content_type: String = row.try_get("content_type").map_err(store_error)?;
    let size: i64 = row.try_get("size").map_err(store_error)?;
    Ok(StoredImage {
        id: attachment_id(&id)?,
        channel_id: ChannelId::parse(&channel_id).map_err(store_error)?,
        uploader: row.try_get("uploader").map_err(store_error)?,
        format: image_format(&content_type)?,
        size: u64::try_from(size).unwrap_or_default(),
    })
}

fn attachment_id(raw: &str) -> Result<AttachmentId, ChatStoreError> {
    AttachmentId::parse(raw).ok_or_else(|| ChatStoreError(format!("invalid image id {raw:?}")))
}

fn image_format(raw: &str) -> Result<ImageFormat, ChatStoreError> {
    ImageFormat::from_content_type(raw)
        .ok_or_else(|| ChatStoreError(format!("invalid image type {raw:?}")))
}

fn store_error(error: impl std::fmt::Display) -> ChatStoreError {
    ChatStoreError(error.to_string())
}
