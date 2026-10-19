-- Members who asked to be pushed when someone replies to one of their posts. A row means yes.
--
-- Pushing a reply to the author means the running server works out, in memory, who wrote the
-- post that was replied to (see "Design constraint" in CLAUDE.md). That is acceptable only for
-- someone who chose it, so the default is no row. This table records nothing about posts: it
-- is the same fact the member could state in a settings page, and a dump of it says who is
-- willing to be pushed, not who wrote what.
--
-- Note what the member is told when they choose: a push service (Google, Mozilla, Apple) sees
-- which device is pushed and when, so it can tell a reply to one of that device's owner's posts
-- from the timing. That is the reason this is off unless asked for.
CREATE TABLE member_reply_push (
    member_id UUID PRIMARY KEY REFERENCES members (id) ON DELETE CASCADE
);
