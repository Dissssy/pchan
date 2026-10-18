-- Moderation was reactive-by-browsing: nothing let a member flag a post, so a panel would
-- have had no inbox.
--
-- No reporter identity is stored. Recording who reported what would create exactly the
-- member↔post link the rest of the schema avoids, and would make reporting something that
-- could be held against you. Abuse is handled by rate limiting at the session layer, which is
-- where posting is already limited.
CREATE TABLE reports (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    post_id     BIGINT      NOT NULL REFERENCES posts (id) ON DELETE CASCADE,
    -- Denormalised so a board moderator's inbox is one indexed read rather than a join
    -- through posts, and so the row survives usefully for ordering even as posts churn.
    board_id    BIGINT      NOT NULL REFERENCES boards (id) ON DELETE CASCADE,
    reason      TEXT        NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ,
    resolved_by UUID        REFERENCES members (id) ON DELETE SET NULL
);

-- The inbox query: unresolved reports for a board, oldest first.
CREATE INDEX reports_open_idx ON reports (board_id, created_at) WHERE resolved_at IS NULL;
CREATE INDEX reports_post_idx ON reports (post_id);

-- One open report per post. A pile-on should not become a pile of rows, and the count of
-- reporters is not information worth keeping given reporters are not recorded.
CREATE UNIQUE INDEX reports_one_open_per_post ON reports (post_id) WHERE resolved_at IS NULL;
