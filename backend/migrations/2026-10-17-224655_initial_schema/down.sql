DROP TRIGGER IF EXISTS cleanup_user_tags_trigger ON user_tags;
DROP TRIGGER IF EXISTS threads_bi ON threads;
DROP TRIGGER IF EXISTS posts_bi_count ON posts;
DROP TRIGGER IF EXISTS posts_bi ON posts;

DROP FUNCTION IF EXISTS cleanup_user_tags();
DROP FUNCTION IF EXISTS delete_oldest_thread();
DROP FUNCTION IF EXISTS update_post_count();
DROP FUNCTION IF EXISTS update_thread();

DROP TABLE IF EXISTS user_tags;
DROP TABLE IF EXISTS member_watching;
DROP TABLE IF EXISTS member_push;
DROP TABLE IF EXISTS request_blocks;
DROP TABLE IF EXISTS join_requests;
DROP TABLE IF EXISTS sessions;
DROP TABLE IF EXISTS identities;
DROP TABLE IF EXISTS members;

DROP TABLE IF EXISTS config;
DROP TABLE IF EXISTS spoilers;
DROP TABLE IF EXISTS banners;
DROP TABLE IF EXISTS files;
DROP TABLE IF EXISTS posts;
DROP TABLE IF EXISTS threads;
DROP TABLE IF EXISTS boards;
