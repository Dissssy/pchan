-- Approval cannot create the member.
--
-- A reviewer only ever sees `request_hash`, which is HMAC(REQUEST_KEY, auth0_sub). Creating
-- the member's identity needs HMAC(IDENTITY_KEY, auth0_sub), and neither value is derivable
-- from the other — that separation is the whole reason a mod cannot connect a join request to
-- the member it becomes. The raw subject only exists during a login, so the member row is
-- created when the approved person next logs in and the request row is consumed then.
--
-- This also decouples member creation order from approval order, so watching the members table
-- does not reveal the sequence in which requests were approved.

ALTER TABLE join_requests ADD COLUMN approved_at TIMESTAMPTZ;

-- Pending queue reads filter on this.
CREATE INDEX join_requests_approved_at_idx ON join_requests (approved_at);
