<!--
delete_oldest_thread
CREATE OR REPLACE FUNCTION delete_oldest_thread() RETURNS TRIGGER AS $$
BEGIN
    IF (SELECT COUNT(*) FROM threads WHERE board = NEW.board) >= 100 THEN
        DELETE FROM threads WHERE board = NEW.board AND latest_post = (SELECT MIN(latest_post) FROM threads WHERE board = NEW.board);
    END IF;
    RETURN NEW;
END;
-->

CREATE OR REPLACE FUNCTION delete_oldest_thread() RETURNS TRIGGER AS $$
BEGIN
    IF (SELECT COUNT(*) FROM threads WHERE board = NEW.board AND id != NEW.id) >= (SELECT value FROM config WHERE key = 'max_threads_per_board') THEN
        DELETE FROM threads WHERE board = NEW.board AND latest_post = (SELECT MIN(latest_post) FROM threads WHERE board = NEW.board AND id != NEW.id);
    END IF;
    RETURN NEW;
END;