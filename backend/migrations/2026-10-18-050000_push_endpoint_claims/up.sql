-- One browser, one member.
--
-- A push endpoint identifies a browser profile, not a login. Before this, a subscription
-- stayed attached to whichever member created it, so after one member logged out and another
-- logged in on the same browser, both members' watched threads were delivered to that browser.
--
-- Subscribing now *claims* the endpoint: any other member's row for it is replaced. Finding
-- that row needs a lookup key, and the data column cannot provide one (a fresh nonce makes
-- every ciphertext different). endpoint_hash is HMAC(PUSH_ENCRYPTION_KEY, "push-endpoint:v1"
-- || endpoint), so a dump does not reveal the endpoint, and because the index is unique it
-- can never show two members sharing a browser, which is what the encryption was protecting.
--
-- Nullable: rows from before this migration are given a hash at startup.
ALTER TABLE member_push ADD COLUMN endpoint_hash TEXT;
CREATE UNIQUE INDEX member_push_endpoint_hash_idx ON member_push (endpoint_hash);
