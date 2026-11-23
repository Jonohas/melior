-- Images uploaded for chat messages. The bytes live in object storage under
-- the id; this row says where the image was uploaded and by whom, which decides
-- who may attach and see it. A channel's images go when the channel is deleted (the
-- objects stay behind in the bucket).
--
-- An image is uploaded before the message that carries it is sent, so it starts
-- out not attached (attached_at_ms NULL). Sending a message with it attaches it.
-- The server removes images that stay unattached longer than
-- UNATTACHED_IMAGE_TTL_SECONDS, row and object.
CREATE TABLE chat_images (
    id             TEXT PRIMARY KEY,
    channel_id     TEXT NOT NULL REFERENCES channels (id) ON DELETE CASCADE,
    uploader       TEXT NOT NULL,
    content_type   TEXT NOT NULL,
    size           BIGINT NOT NULL,
    -- Both from the database clock, so every server instance agrees on them.
    uploaded_at_ms BIGINT NOT NULL DEFAULT (extract(epoch FROM clock_timestamp()) * 1000)::bigint,
    attached_at_ms BIGINT
);

-- Finding the images that were never attached, oldest first.
CREATE INDEX chat_images_unattached ON chat_images (uploaded_at_ms) WHERE attached_at_ms IS NULL;

-- A message may carry one attachment: an uploaded image or a GIF link. Its text
-- (body) is then allowed to be empty.
ALTER TABLE chat_messages
    ADD COLUMN image_id   TEXT REFERENCES chat_images (id) ON DELETE CASCADE,
    ADD COLUMN gif_url    TEXT,
    ADD COLUMN gif_width  INTEGER,
    ADD COLUMN gif_height INTEGER,
    ADD CONSTRAINT chat_messages_one_attachment CHECK (image_id IS NULL OR gif_url IS NULL),
    ADD CONSTRAINT chat_messages_gif_size CHECK (
        (gif_url IS NULL AND gif_width IS NULL AND gif_height IS NULL)
        OR (gif_url IS NOT NULL AND gif_width IS NOT NULL AND gif_height IS NOT NULL)
    );
