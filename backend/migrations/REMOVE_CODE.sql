-- migration to create a column in the posts table called moderator that is a boolean and not null default false
-- then set the value of the moderator column to true for all posts where code is not null
-- then remove the code column from the posts table

ALTER TABLE posts ADD COLUMN moderator BOOLEAN NOT NULL DEFAULT FALSE;
UPDATE posts SET moderator = TRUE WHERE code IS NOT NULL;
ALTER TABLE posts DROP COLUMN code;