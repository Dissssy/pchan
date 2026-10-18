-- pchan schema, v2.
--
-- Content tables (boards/threads/posts/files/banners/spoilers/config) reproduce the
-- pre-migration shape. The identity layer is new: see `members` onward.
--
-- Anonymity invariants this schema is built around:
--   * `members` carries no identifying column and no created_at. Row creation order must not
--     be a signal, hence a random UUID primary key rather than a sequence.
--   * `identities.identity_hash` and `join_requests.request_hash` both derive from the same
--     Auth0 `sub` but under SEPARATE HMAC keys, so holding this database does not let anyone
--     join the join queue to the member it produced.
--   * `posts.actual_author` and `user_tags.invite_hash` are HMACs under keys held outside the
--     database. The previous plain-SHA256 forms were recoverable by brute force from a dump.

-- gen_random_uuid() is core since PG13; no pgcrypto needed.

-- ---------------------------------------------------------------- content

CREATE TABLE boards (
    id            BIGSERIAL PRIMARY KEY,
    name          TEXT    NOT NULL,
    discriminator TEXT    NOT NULL UNIQUE,
    post_count    BIGINT  NOT NULL DEFAULT 0,
    private       BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE threads (
    id          BIGSERIAL PRIMARY KEY,
    board       BIGINT NOT NULL REFERENCES boards (id) ON UPDATE CASCADE ON DELETE CASCADE,
    post_id     BIGINT NOT NULL,
    latest_post BIGINT NOT NULL,
    topic       TEXT   NOT NULL
);
CREATE INDEX threads_board_idx ON threads (board);
CREATE INDEX threads_latest_post_idx ON threads (board, latest_post);

CREATE TABLE posts (
    id            BIGSERIAL PRIMARY KEY,
    post_number   BIGINT  NOT NULL,
    thread        BIGINT  NOT NULL REFERENCES threads (id) ON UPDATE CASCADE ON DELETE CASCADE,
    board         BIGINT  NOT NULL REFERENCES boards (id) ON UPDATE CASCADE ON DELETE CASCADE,
    author        TEXT,
    content       TEXT    NOT NULL,
    "timestamp"   TIMESTAMP NOT NULL,
    replies_to    BIGINT[] NOT NULL DEFAULT '{}',
    -- HMAC(POST_AUTHOR_KEY, member.author_seed || post id). Rotating a member's author_seed
    -- deliberately orphans these, which is how a member sheds their own history.
    actual_author TEXT    NOT NULL,
    moderator     BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE INDEX posts_thread_idx ON posts (thread);
CREATE INDEX posts_board_idx ON posts (board);
CREATE INDEX posts_actual_author_idx ON posts (actual_author);

CREATE TABLE files (
    id       BIGINT  NOT NULL PRIMARY KEY
                 REFERENCES posts (id) ON UPDATE CASCADE ON DELETE CASCADE,
    filepath TEXT    NOT NULL,
    hash     TEXT    NOT NULL,
    spoiler  BOOLEAN NOT NULL
);
CREATE INDEX files_hash_idx ON files (hash);

CREATE TABLE banners (
    id       BIGSERIAL PRIMARY KEY,
    img_path TEXT NOT NULL,
    href     TEXT,
    boards   BIGINT[]
);

-- id is BIGSERIAL and boards is NOT NULL to match how the code has always read them; the
-- previous database had `integer` and a nullable array while the Rust schema declared BigInt
-- and a non-null array.
CREATE TABLE spoilers (
    id       BIGSERIAL PRIMARY KEY,
    img_path TEXT NOT NULL,
    boards   BIGINT[] NOT NULL DEFAULT '{}'
);

CREATE TABLE config (
    key   TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);

-- delete_oldest_thread() CASTs this value to int. If the row is absent the comparison yields
-- NULL, the loop never exits, and it deletes every thread on the board. Seed it, always.
INSERT INTO config (key, value) VALUES ('max_threads_per_board', '100');

-- ---------------------------------------------------------------- identity

CREATE TABLE members (
    id          UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Random per member; the preimage for post authorship. Rotatable by the member.
    author_seed TEXT    NOT NULL UNIQUE,
    role        TEXT    NOT NULL DEFAULT 'member',
    banned      BOOLEAN NOT NULL DEFAULT FALSE,
    CONSTRAINT members_role_valid CHECK (role IN ('member', 'site_mod', 'admin'))
);

-- A member may hold several identities (e.g. an Auth0 account with Discord linked to it),
-- so the hash is the key and member_id is non-unique.
CREATE TABLE identities (
    identity_hash TEXT NOT NULL PRIMARY KEY,
    member_id     UUID NOT NULL REFERENCES members (id) ON DELETE CASCADE
);
CREATE INDEX identities_member_id_idx ON identities (member_id);

CREATE TABLE sessions (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    token_hash TEXT        NOT NULL UNIQUE,
    member_id  UUID        NOT NULL REFERENCES members (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    last_seen  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX sessions_member_id_idx ON sessions (member_id);
CREATE INDEX sessions_expires_at_idx ON sessions (expires_at);

-- Holds pending requests only; a row is deleted on approval or decline so the disclosure
-- text does not outlive the decision.
CREATE TABLE join_requests (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    request_hash TEXT        NOT NULL UNIQUE,
    disclosure   TEXT        NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Declined or removed. `until` is a TTL: a permanent row would be a permanent pseudonym for
-- that identity, which is the thing this schema is trying not to keep.
CREATE TABLE request_blocks (
    request_hash TEXT        NOT NULL PRIMARY KEY,
    until        TIMESTAMPTZ NOT NULL
);

-- One row per browser/device. Encrypted under PUSH_ENCRYPTION_KEY: a push endpoint URL is a
-- stable per-profile identifier, so in plaintext two member rows sharing one would prove the
-- same person.
CREATE TABLE member_push (
    id        UUID  PRIMARY KEY DEFAULT gen_random_uuid(),
    member_id UUID  NOT NULL REFERENCES members (id) ON DELETE CASCADE,
    data      BYTEA NOT NULL
);
CREATE INDEX member_push_member_id_idx ON member_push (member_id);

-- Replaces members.watching. The thread FK's ON DELETE CASCADE does what the old
-- remove_from_watching() trigger did, so that trigger is intentionally not recreated.
CREATE TABLE member_watching (
    member_id UUID   NOT NULL REFERENCES members (id) ON DELETE CASCADE,
    thread_id BIGINT NOT NULL REFERENCES threads (id) ON DELETE CASCADE,
    PRIMARY KEY (member_id, thread_id)
);
CREATE INDEX member_watching_thread_id_idx ON member_watching (thread_id);

-- Per-board access and moderation grants. invite_hash NULL means an unredeemed invite.
-- invite_hash = HMAC(INVITE_KEY, member id || board id); generated_by is the inviter's
-- invite_hash, or the literal 'ADMIN'. Keyed on member id, not author_seed, so rotating a
-- member's posting identity does not cost them board access.
CREATE TABLE user_tags (
    id           UUID   PRIMARY KEY DEFAULT gen_random_uuid(),
    board_id     BIGINT NOT NULL REFERENCES boards (id) ON DELETE CASCADE,
    user_name    TEXT   NOT NULL,
    invite_hash  TEXT   UNIQUE,
    tag_kind     TEXT   NOT NULL,
    generated_by TEXT   NOT NULL,
    CONSTRAINT user_tags_kind_valid CHECK (tag_kind IN ('INVITE', 'MODERATOR'))
);
CREATE INDEX user_tags_board_id_idx ON user_tags (board_id);
CREATE INDEX user_tags_lookup_idx ON user_tags (board_id, tag_kind, invite_hash);

-- ---------------------------------------------------------------- triggers

CREATE FUNCTION update_thread() RETURNS TRIGGER AS $$
BEGIN
    IF (SELECT COUNT(*) FROM posts WHERE thread = NEW.thread) <= 300 THEN
        UPDATE threads SET latest_post = NEW.id WHERE id = NEW.thread;
    END IF;
    IF (SELECT COUNT(*) FROM posts WHERE thread = NEW.thread) = 0 THEN
        UPDATE threads SET post_id = NEW.id WHERE id = NEW.thread;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER posts_bi BEFORE INSERT ON posts
    FOR EACH ROW EXECUTE FUNCTION update_thread();

CREATE FUNCTION update_post_count() RETURNS TRIGGER AS $$
BEGIN
    UPDATE boards SET post_count = NEW.post_number WHERE id = NEW.board;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER posts_bi_count BEFORE INSERT ON posts
    FOR EACH ROW EXECUTE FUNCTION update_post_count();

CREATE FUNCTION delete_oldest_thread() RETURNS TRIGGER AS $$
BEGIN
    LOOP
        IF (SELECT COUNT(*) FROM threads WHERE board = NEW.board)
           < (SELECT CAST((SELECT value FROM config WHERE key = 'max_threads_per_board') AS int)) THEN
            EXIT;
        END IF;
        DELETE FROM threads
            WHERE board = NEW.board
              AND latest_post = (SELECT MIN(latest_post) FROM threads WHERE board = NEW.board);
    END LOOP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER threads_bi BEFORE INSERT ON threads
    FOR EACH ROW EXECUTE FUNCTION delete_oldest_thread();

-- Deletes invite subtrees whose parent grant is gone. Note for any future re-keying of
-- invite_hash: this fires per row, so rewriting hashes row-by-row cascade-deletes children
-- whose generated_by still points at the old parent value. Disable it around such a rewrite.
CREATE FUNCTION cleanup_user_tags() RETURNS TRIGGER AS $$
BEGIN
    DELETE FROM user_tags ut1
        WHERE generated_by IS NOT NULL
          AND generated_by != 'ADMIN'
          AND NOT EXISTS (
              SELECT 1 FROM user_tags ut2 WHERE ut2.invite_hash = ut1.generated_by
          );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER cleanup_user_tags_trigger AFTER INSERT OR UPDATE OR DELETE ON user_tags
    FOR EACH ROW EXECUTE FUNCTION cleanup_user_tags();
