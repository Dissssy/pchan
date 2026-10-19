DROP INDEX IF EXISTS member_push_endpoint_hash_idx;
ALTER TABLE member_push DROP COLUMN IF EXISTS endpoint_hash;
