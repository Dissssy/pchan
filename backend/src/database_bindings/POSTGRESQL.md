<!--
delete_oldest_thread
CREATE OR REPLACE FUNCTION delete_oldest_thread() RETURNS TRIGGER AS $$
BEGIN
    LOOP
        IF (SELECT COUNT(*) FROM threads WHERE board = NEW.board) < (SELECT CAST((SELECT value FROM config WHERE key = 'max_threads_per_board') AS int)) THEN
            EXIT;
        END IF;
        DELETE FROM threads WHERE board = NEW.board AND latest_post = (SELECT MIN(latest_post) FROM threads WHERE board = NEW.board);
    END LOOP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
-->
