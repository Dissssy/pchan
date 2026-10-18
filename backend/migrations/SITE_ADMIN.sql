-- modify the members table to add an admin column
ALTER TABLE members ADD COLUMN admin BOOLEAN NOT NULL DEFAULT FALSE;
-- migrate the moderates column to the admin column, if it's not null the admin column should be true
UPDATE members SET admin = TRUE WHERE moderates IS NOT NULL;
-- drop the moderates column
ALTER TABLE members DROP COLUMN moderates;
