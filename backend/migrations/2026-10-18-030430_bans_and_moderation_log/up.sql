-- Bans move off the member row, and moderation actions become auditable.

-- A ban is a moderation action, so it wants an actor, a reason and an expiry — none of which
-- a boolean column can hold. It is a table rather than an array of board ids on `members` for
-- the same reason `watching` became `member_watching` in this migration: array mutation means
-- read-modify-write. Unban is now a delete.
CREATE TABLE bans (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    member_id  UUID        NOT NULL REFERENCES members (id) ON DELETE CASCADE,
    -- NULL means sitewide. A board id means that board only.
    board_id   BIGINT      REFERENCES boards (id) ON DELETE CASCADE,
    issued_by  UUID        NOT NULL REFERENCES members (id),
    reason     TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- NULL means permanent.
    expires_at TIMESTAMPTZ
);

CREATE INDEX bans_member_id_idx ON bans (member_id);
CREATE UNIQUE INDEX bans_member_board_idx ON bans (member_id, board_id);
-- Postgres treats NULLs as distinct, so the index above does NOT prevent two sitewide bans on
-- the same member. This does.
CREATE UNIQUE INDEX bans_member_sitewide_idx ON bans (member_id) WHERE board_id IS NULL;

-- Actions taken in an official capacity are attributable; posting is not. Those are separate
-- properties, and recording the acting moderator is what makes it possible to revoke a
-- moderator who misbehaves without weakening poster anonymity.
--
-- `actor` is also exactly the handle revocation needs, since board moderator status is a
-- `user_tags` row.
--
-- The affected member's id is deliberately absent. The ban row already identifies them, and
-- repeating it here would only widen the correlation noted below.
CREATE TABLE moderation_log (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    actor      UUID        NOT NULL REFERENCES members (id) ON DELETE CASCADE,
    action     TEXT        NOT NULL,
    board_id   BIGINT      REFERENCES boards (id) ON DELETE SET NULL,
    -- Opaque reference: a post number, a request id, a board discriminator. Never a member id.
    target     TEXT,
    detail     TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX moderation_log_actor_idx ON moderation_log (actor, created_at DESC);
CREATE INDEX moderation_log_board_idx ON moderation_log (board_id, created_at DESC);
CREATE INDEX moderation_log_created_idx ON moderation_log (created_at DESC);

-- Accepted exception to the anonymity model: a ban entry correlates a post to a member by
-- timing, since the log names the post and a `bans` row appears at the same moment. This is
-- unavoidable if ban actions are reviewable at all. It is bounded because `reason` carries
-- routine accountability (so the default view need not show both halves together), board scope
-- narrows the candidate set, and it only ever exposes members who were banned. The log is
-- therefore admin-only; moderators see only their own entries.

-- Superseded by the bans table. Sitewide bans are `board_id IS NULL`.
ALTER TABLE members DROP COLUMN banned;
