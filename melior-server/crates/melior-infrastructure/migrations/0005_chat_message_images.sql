-- A message carries up to 10 images, in order, instead of at most one. The
-- one-image column moves into this table. A GIF stays in chat_messages: a
-- message with a GIF has no images (the server's rule; not checked here).
CREATE TABLE chat_message_images (
    message_id BIGINT NOT NULL REFERENCES chat_messages (id) ON DELETE CASCADE,
    position   SMALLINT NOT NULL,
    image_id   TEXT NOT NULL REFERENCES chat_images (id) ON DELETE CASCADE,
    PRIMARY KEY (message_id, position)
);

-- Deleting an image record finds the messages that carry it.
CREATE INDEX chat_message_images_image_id ON chat_message_images (image_id);

INSERT INTO chat_message_images (message_id, position, image_id)
    SELECT id, 0, image_id FROM chat_messages WHERE image_id IS NOT NULL;

ALTER TABLE chat_messages DROP CONSTRAINT chat_messages_one_attachment;
ALTER TABLE chat_messages DROP COLUMN image_id;
