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

/// A message's columns, with its images' ids and media types in order (`imgs`,
/// from [`IMAGES_JOIN`]).
const COLUMNS: &str = "m.id, m.channel_id, m.author, m.body, m.sent_at_ms, m.gif_url, \
                       m.gif_width, m.gif_height, imgs.ids AS image_ids, imgs.types AS image_types";

/// The images of message `m`, as two arrays in their order; NULL for none.
const IMAGES_JOIN: &str = "LEFT JOIN LATERAL (
    SELECT array_agg(i.id ORDER BY mi.position) AS ids,
           array_agg(i.content_type ORDER BY mi.position) AS types
    FROM chat_message_images mi JOIN chat_images i ON i.id = mi.image_id
    WHERE mi.message_id = m.id
) imgs ON true";

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
        let image_ids: Vec<&str> =
            content.image_ids().into_iter().map(AttachmentId::as_str).collect();
        let gif = content.attachments().iter().find_map(|attachment| match attachment {
            Attachment::Gif(gif) => Some(gif),
            Attachment::Image { .. } => None,
        });
        // One statement: the message, its images in order, and those images
        // marked attached. The images' foreign keys lock their records until
        // this commits, so a sweep running at the same time leaves them.
        let row = sqlx::query(
            "WITH m AS (
                INSERT INTO chat_messages (channel_id, author, body, gif_url, gif_width, gif_height)
                VALUES ($1, $2, $3, $4, $5, $6)
                RETURNING id, sent_at_ms
             ), linked AS (
                INSERT INTO chat_message_images (message_id, position, image_id)
                SELECT m.id, (x.ord - 1)::smallint, x.id
                FROM m, unnest($7::text[]) WITH ORDINALITY AS x(id, ord)
             ), attached AS (
                UPDATE chat_images
                SET attached_at_ms = (extract(epoch FROM clock_timestamp()) * 1000)::bigint
                WHERE id = ANY($7::text[]) AND attached_at_ms IS NULL
             )
             SELECT id, sent_at_ms FROM m",
        )
        .bind(channel_id.as_str())
        .bind(author)
        .bind(content.text_str())
        .bind(gif.map(GifLink::url))
        .bind(gif.map(|gif| i32::from(gif.width())))
        .bind(gif.map(|gif| i32::from(gif.height())))
        .bind(&image_ids)
        .fetch_one(pool)
        .await
        .map_err(store_error)?;
        Ok(ChatMessage {
            id: MessageId::new(row.try_get("id").map_err(store_error)?),
            channel_id: channel_id.clone(),
            author: author.to_owned(),
            content: content.clone(),
            sent_at_ms: row.try_get("sent_at_ms").map_err(store_error)?,
        })
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
            "SELECT {COLUMNS} FROM chat_messages m {IMAGES_JOIN}
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

    async fn images(&self, ids: &[AttachmentId]) -> Result<Vec<StoredImage>, ChatStoreError> {
        let pool = self.database.pool().await.map_err(store_error)?;
        let ids: Vec<&str> = ids.iter().map(AttachmentId::as_str).collect();
        sqlx::query(
            "SELECT id, channel_id, uploader, content_type, size FROM chat_images
             WHERE id = ANY($1::text[])",
        )
        .bind(&ids)
        .fetch_all(pool)
        .await
        .map_err(store_error)?
        .iter()
        .map(image_from_row)
        .collect()
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

/// Rebuilds a message from a row. Stored values go through the same validation
/// as user input: a row that does not fit the domain is an error, not a message.
fn message_from_row(row: &PgRow) -> Result<ChatMessage, ChatStoreError> {
    let id: i64 = row.try_get("id").map_err(store_error)?;
    let channel_id: String = row.try_get("channel_id").map_err(store_error)?;
    let author: String = row.try_get("author").map_err(store_error)?;
    let body: String = row.try_get("body").map_err(store_error)?;
    let sent_at_ms: i64 = row.try_get("sent_at_ms").map_err(store_error)?;

    let text = ChatText::parse_optional(&body).map_err(store_error)?;
    let content = ChatContent::new(text, attachments_from_row(row)?).map_err(store_error)?;
    Ok(ChatMessage {
        id: MessageId::new(id),
        channel_id: ChannelId::parse(&channel_id).map_err(store_error)?,
        author,
        content,
        sent_at_ms,
    })
}

/// The images of a row in order, then its GIF; [`ChatContent::new`] checks
/// that they go together.
fn attachments_from_row(row: &PgRow) -> Result<Vec<Attachment>, ChatStoreError> {
    let ids: Option<Vec<String>> = row.try_get("image_ids").map_err(store_error)?;
    let types: Option<Vec<String>> = row.try_get("image_types").map_err(store_error)?;
    let mut attachments = ids
        .unwrap_or_default()
        .iter()
        .zip(types.unwrap_or_default())
        .map(|(id, content_type)| {
            Ok(Attachment::Image { id: attachment_id(id)?, format: image_format(&content_type)? })
        })
        .collect::<Result<Vec<_>, ChatStoreError>>()?;
    let gif_url: Option<String> = row.try_get("gif_url").map_err(store_error)?;
    if let Some(gif_url) = gif_url {
        let width: Option<i32> = row.try_get("gif_width").map_err(store_error)?;
        let height: Option<i32> = row.try_get("gif_height").map_err(store_error)?;
        let side = |value: Option<i32>| value.and_then(|v| u32::try_from(v).ok()).unwrap_or(0);
        let gif = GifLink::parse(&gif_url, side(width), side(height)).map_err(store_error)?;
        attachments.push(Attachment::Gif(gif));
    }
    Ok(attachments)
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
