-- The room registry: who owns a room and who may enter it.
CREATE TABLE rooms (
    channel_id     TEXT PRIMARY KEY,
    owner_id       TEXT NOT NULL,
    owner_username TEXT NOT NULL,
    restricted     BOOLEAN NOT NULL DEFAULT FALSE,
    -- Invited usernames, lower-case.
    members        TEXT[] NOT NULL DEFAULT '{}',
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX rooms_owner_id ON rooms (owner_id);
CREATE INDEX rooms_members ON rooms USING GIN (members);
