-- postgresql
-- add a field called generated_by which will be a reference to the invite_hash column
ALTER TABLE user_tags ADD COLUMN generated_by TEXT NOT NULL DEFAULT 'ADMIN';

-- remove the default from the generated_by column
ALTER TABLE user_tags ALTER COLUMN generated_by DROP DEFAULT;

-- make the invite_hash column unique
ALTER TABLE user_tags ADD CONSTRAINT unique_invite_hash UNIQUE (invite_hash);

-- we can't foreign key to the invite_hash column because it's not unique and can be null so just on_update on_delete run a function to clean up the table
CREATE OR REPLACE FUNCTION cleanup_user_tags() RETURNS TRIGGER AS $$
BEGIN
    -- find all the rows that have a value in generated_by that does not exist in invite_hash on any row, ignore where generated_by is "ADMIN"
    DELETE FROM user_tags ut1 
        WHERE generated_by IS NOT NULL 
        AND generated_by != 'ADMIN' 
        AND NOT EXISTS (
            SELECT 1 
            FROM user_tags ut2 
            WHERE ut2.invite_hash = ut1.generated_by
        );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER cleanup_user_tags_trigger
    AFTER INSERT OR UPDATE OR DELETE ON user_tags
    FOR EACH ROW EXECUTE FUNCTION cleanup_user_tags();
