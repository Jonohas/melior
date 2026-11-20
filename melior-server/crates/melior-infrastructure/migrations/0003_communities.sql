-- Communities: roles, members, categories, channels (formerly rooms), permission
-- overrides and invites. Existing rooms move into one community named "Melior".

CREATE TABLE communities (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    owner_id       TEXT NOT NULL,
    owner_username TEXT NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- The @everyone role's id is its community's id.
CREATE TABLE roles (
    id           TEXT PRIMARY KEY,
    community_id TEXT NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    color        INTEGER,
    position     INTEGER NOT NULL,
    permissions  BIGINT NOT NULL
);
CREATE INDEX roles_community_id ON roles (community_id);

CREATE TABLE members (
    community_id TEXT NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    user_id      TEXT NOT NULL,
    username     TEXT NOT NULL,
    joined_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (community_id, user_id)
);
CREATE INDEX members_user_id ON members (user_id);

CREATE TABLE member_roles (
    community_id TEXT NOT NULL,
    user_id      TEXT NOT NULL,
    role_id      TEXT NOT NULL REFERENCES roles (id) ON DELETE CASCADE,
    PRIMARY KEY (community_id, user_id, role_id),
    FOREIGN KEY (community_id, user_id) REFERENCES members (community_id, user_id) ON DELETE CASCADE
);

CREATE TABLE categories (
    id           TEXT PRIMARY KEY,
    community_id TEXT NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    position     INTEGER NOT NULL
);
CREATE INDEX categories_community_id ON categories (community_id);

CREATE TABLE category_overrides (
    category_id TEXT NOT NULL REFERENCES categories (id) ON DELETE CASCADE,
    target_kind TEXT NOT NULL CHECK (target_kind IN ('role', 'member')),
    target_id   TEXT NOT NULL,
    allow       BIGINT NOT NULL,
    deny        BIGINT NOT NULL,
    PRIMARY KEY (category_id, target_kind, target_id)
);

-- A room becomes a channel. chat_messages.channel_id keeps its foreign key: it
-- follows the renamed table and column.
ALTER TABLE rooms RENAME TO channels;
ALTER TABLE channels RENAME COLUMN channel_id TO id;
ALTER INDEX rooms_pkey RENAME TO channels_pkey;
ALTER TABLE channels
    ADD COLUMN community_id TEXT REFERENCES communities (id) ON DELETE CASCADE,
    ADD COLUMN name         TEXT,
    ADD COLUMN topic        TEXT NOT NULL DEFAULT '',
    ADD COLUMN kind         TEXT NOT NULL DEFAULT 'voice' CHECK (kind IN ('voice', 'text')),
    ADD COLUMN category_id  TEXT REFERENCES categories (id) ON DELETE SET NULL,
    ADD COLUMN position     INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN synced       BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN media_mode   TEXT NOT NULL DEFAULT 'auto'
        CHECK (media_mode IN ('auto', 'peer_to_peer', 'server')),
    ADD COLUMN user_limit   SMALLINT;

CREATE TABLE channel_overrides (
    channel_id  TEXT NOT NULL REFERENCES channels (id) ON DELETE CASCADE,
    target_kind TEXT NOT NULL CHECK (target_kind IN ('role', 'member')),
    target_id   TEXT NOT NULL,
    allow       BIGINT NOT NULL,
    deny        BIGINT NOT NULL,
    PRIMARY KEY (channel_id, target_kind, target_id)
);

CREATE TABLE invites (
    code          TEXT PRIMARY KEY,
    community_id  TEXT NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    created_by    TEXT NOT NULL,
    creator_id    TEXT NOT NULL,
    -- Milliseconds since the Unix epoch; NULL never expires.
    expires_at_ms BIGINT,
    max_uses      INTEGER,
    uses          INTEGER NOT NULL DEFAULT 0,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX invites_community_id ON invites (community_id);

-- Existing rooms, if any, join one community owned by the owner of the oldest
-- room. Every room owner becomes a member; @everyone gets the default
-- permissions (view_channel, connect, send_messages, read_history,
-- create_invite = 31); a restricted room is hidden from @everyone and shown to
-- its owner. Invitation lists hold usernames, not user ids, and are dropped.
DO $$
DECLARE
    community TEXT := gen_random_uuid()::text;
    oldest    RECORD;
BEGIN
    SELECT owner_id, owner_username INTO oldest FROM channels ORDER BY created_at, id LIMIT 1;
    IF NOT FOUND THEN
        RETURN;
    END IF;

    INSERT INTO communities (id, name, owner_id, owner_username)
    VALUES (community, 'Melior', oldest.owner_id, oldest.owner_username);

    INSERT INTO roles (id, community_id, name, color, position, permissions)
    VALUES (community, community, '@everyone', NULL, 0, 31);

    INSERT INTO members (community_id, user_id, username)
    SELECT DISTINCT ON (owner_id) community, owner_id, owner_username
    FROM channels
    ORDER BY owner_id, created_at;

    INSERT INTO invites (code, community_id, created_by, creator_id)
    SELECT string_agg(
               substr('ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789',
                      1 + floor(random() * 62)::int, 1), ''),
           community, oldest.owner_username, oldest.owner_id
    FROM generate_series(1, 10);

    UPDATE channels AS c
    SET community_id = community, name = c.id, kind = 'voice', media_mode = 'auto',
        synced = FALSE, position = ordered.position
    FROM (SELECT id, (row_number() OVER (ORDER BY created_at, id) - 1)::int AS position
          FROM channels) AS ordered
    WHERE ordered.id = c.id;

    INSERT INTO channel_overrides (channel_id, target_kind, target_id, allow, deny)
    SELECT id, 'role', community, 0, 1 FROM channels WHERE restricted;
    INSERT INTO channel_overrides (channel_id, target_kind, target_id, allow, deny)
    SELECT id, 'member', owner_id, 1, 0 FROM channels WHERE restricted;
END
$$;

ALTER TABLE channels
    ALTER COLUMN community_id SET NOT NULL,
    ALTER COLUMN name SET NOT NULL,
    DROP COLUMN owner_id,
    DROP COLUMN owner_username,
    DROP COLUMN restricted,
    DROP COLUMN members;
CREATE INDEX channels_community_id ON channels (community_id);
