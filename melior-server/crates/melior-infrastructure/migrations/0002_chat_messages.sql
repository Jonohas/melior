-- What was said in a room. A room's messages go when the room is deleted.
CREATE TABLE chat_messages (
    id         BIGSERIAL PRIMARY KEY,
    channel_id TEXT NOT NULL REFERENCES rooms (channel_id) ON DELETE CASCADE,
    author     TEXT NOT NULL,
    body       TEXT NOT NULL,
    -- Milliseconds since the Unix epoch, taken from the database clock so that
    -- every server instance agrees on it.
    sent_at_ms BIGINT NOT NULL DEFAULT (extract(epoch FROM clock_timestamp()) * 1000)::bigint
);

-- Paging back through a room's history: newest first, from an id downwards.
CREATE INDEX chat_messages_channel_id_id ON chat_messages (channel_id, id DESC);
