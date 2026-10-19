DROP INDEX IF EXISTS join_requests_approved_at_idx;
ALTER TABLE join_requests DROP COLUMN IF EXISTS approved_at;
