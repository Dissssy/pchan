ALTER TABLE members ADD COLUMN banned BOOLEAN NOT NULL DEFAULT FALSE;
-- Collapse what the boolean can express: sitewide bans only.
UPDATE members SET banned = TRUE
  WHERE id IN (SELECT member_id FROM bans WHERE board_id IS NULL);
DROP TABLE IF EXISTS moderation_log;
DROP TABLE IF EXISTS bans;
