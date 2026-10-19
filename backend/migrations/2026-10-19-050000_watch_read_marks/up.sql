-- "Read through post N" for each watched thread.
--
-- Unread notifications are derived, not stored: a watched thread has unread posts when it holds
-- posts numbered above last_read. There is deliberately no row per notification. The poster is
-- never notified of their own post, so per-notification rows would show, for every post, one
-- watcher with no row: the author. A single watermark has no such gap, and a poster's value
-- (they were looking at the thread, newest post last) is the same as any reader's who stopped
-- at that post.
--
-- Existing watches start fully read, so nobody opens the site to a flood of old threads.
ALTER TABLE member_watching ADD COLUMN last_read BIGINT NOT NULL DEFAULT 0;

UPDATE member_watching mw
SET last_read = COALESCE((SELECT max(p.post_number) FROM posts p WHERE p.thread = mw.thread_id), 0);
