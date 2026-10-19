-- rename board_access to user_tags
-- rename invite_name to user_name
-- add a field called tag_kind which will be an enum in code but stored as a string in the database

ALTER TABLE board_access RENAME TO user_tags;
ALTER TABLE user_tags RENAME COLUMN invite_name TO user_name;
ALTER TABLE user_tags ADD COLUMN tag_kind TEXT NOT NULL DEFAULT 'INVITE';

-- remove the default from the tag_kind column
ALTER TABLE user_tags ALTER COLUMN tag_kind DROP DEFAULT;