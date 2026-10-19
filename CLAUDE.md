# pchan

Rust imageboard. Workspace: `backend` (warp + diesel-async), `frontend` (Yew/WASM via trunk),
`database` (schema + queries), `common` (shared types), `profanity`.

## Running it after you change something

The server does **not** reload. Backend changes need a rebuild and restart; frontend changes
need a trunk build but no restart, because the backend serves `frontend/dist` from disk on
every request.

### Backend change

```sh
cd /git/pchan
cargo build -p backend

# Stop the old one. Do NOT use `pkill -f target/debug/backend`: that pattern matches the
# shell command running it, so the shell kills itself and the command dies mid-script.
pgrep -x backend | while read p; do kill "$p"; done; sleep 2

# setsid + </dev/null detaches it, so it survives the shell that started it.
RUST_LOG=info setsid ./target/debug/backend > /tmp/pchan.log 2>&1 < /dev/null &
sleep 8

ss -ltnp | grep 16835 || tail -20 /tmp/pchan.log   # confirm, or find out why not
```

### Frontend change

```sh
cd /git/pchan/frontend && trunk build --release
```

No restart. The bundle filename is content-hashed, so a hard refresh picks it up; a plain
refresh may serve a cached `index.html`.

### Both

Build the frontend first, then rebuild and restart the backend.

## Configuration

Two separate mechanisms, and mixing them up wastes time:

- **`backend/.env`** — runtime. `DATABASE_URL`, Auth0 credentials, the five HMAC keys,
  `VAPID_PRIVATE_KEY` (web push signing), `PUBLIC_URL`, `COOKIE_SECURE`. Loaded at startup; validation refuses to boot on a missing,
  short or reused key. Changing these needs only a restart. See `backend/.env.example`.
  Discovery order: `PCHAN_ENV_FILE`, then `.env` in the working directory or a parent, then
  `backend/.env` relative to the working directory, then `.env` beside the executable — so
  running from the repo root works.
- **`.cargo/config.toml` `[env]`** — compile-time `env!()`: `PORT`, `FILE_STORAGE_PATH`,
  `TRASH_STORAGE_PATH`, `DISTRIBUTION_PATH`, `TRIM_TIME`, and legacy secrets. Changing these
  needs a **rebuild**, not a restart.

Cargo's `[env]` has no `force = true`, so real environment variables win **at build time**.
That is how to build an isolated instance without editing any config:

```sh
PORT=16899 FILE_STORAGE_PATH=/tmp/t/files TRASH_STORAGE_PATH=/tmp/t/trash \
  cargo build -p backend
DATABASE_URL=postgres://postgres:disclone@localhost:5432/some_test_db \
  ./target/debug/backend
```

## Databases

Postgres 14 in the **shared** `postgresql-db-1` container (host port 5432) alongside ~18
unrelated services. Always name the database explicitly; never do anything cluster-wide.

- `pchan_v2` — current. `backend/.env` points here.
- `pchan` — the pre-migration database, kept for reference.
- `pchan_dev` — **the test database.** Destructive tests and any migration are fine here (the owner
  said so on 2026-10-19); it was rebuilt from the migrations that day and holds only synthetic
  rows. Nothing else in the cluster is, including `pchan_claude_test`, which was not named for this.
  To reset it: `drop schema public cascade; create schema public;` then `diesel migration run`
  against it.

```sh
docker exec postgresql-db-1 psql -U postgres -d pchan_v2 -c "select ..."
docker exec -i postgresql-db-1 psql -U postgres -d pchan_v2 < file.sql   # -i for stdin
```

Migrations are diesel, tracked in `backend/migrations/`, and regenerate `database/src/raw.rs`:

```sh
cd backend
DATABASE_URL=postgres://postgres:disclone@localhost:5432/pchan_v2 diesel migration run
```

`backend/src/schema.patch` is reapplied automatically on regeneration: Postgres cannot express
"array of non-null elements", so `print-schema` emits `Array<Nullable<Int8>>` for every
`BIGINT[]` and the patch restores `Array<Int8>`.

## Live updates and notifications

How a post reaches people. Read this before changing any of it; most of it exists because of a bug.

**The stream.** Each tab keeps one server-sent-event connection, `GET /api/v1/events?thread=<board>/<n>`
(`backend/src/push.rs`, the `Hub`; `frontend/src/events.rs` on the page). Events are `new_post`,
`post_deleted`, `inbox_changed` and `resync`. Each has an id (`<boot>-<n>`); the last 500 are kept in
memory so a reconnect (`Last-Event-ID`, or `?last=` when the page opens a fresh connection) replays
what it missed, or gets `resync` if it cannot. Per-connection queues are bounded: a stuck one is
closed and recovers by replaying. Logout and bans close that member's streams.

**Unread is derived, never stored per notification.** `member_watching.last_read` is a "read through
post N" watermark. The inbox (`GET /api/v1/notifications`) is computed from it. A row per notification
would show, for every post, one watcher without a row, which is the author. Reading a thread, the
toast X, "mark read" and the push's "Go away." all `PUT .../read`; the server then sends
`inbox_changed` to the member's other tabs.

**Attention tiers (who gets what).** Each tab reports *facts* (`PUT /api/v1/presence`: visible, which
thread it shows, seconds since last touched). The server decides (`Hub::attention`) from the tab the
member touched most recently, so the answer degrades by itself as time passes: **Reading** that thread
(touched within 2 min) gets nothing and the post counts as read; **Active** on the site (within 10 min)
gets an in-page notification and no push; **Away** gets a push too. `dispatch_push_notifications` in
`database_bindings/mod.rs` applies it. Reports expire after 45s if a tab vanishes.

**Replies.** Every reply is sent to every connected member who can see the board; the *browser*
decides whether it is to one of their posts, from `frontend/src/authored.rs` (a localStorage cache
filled by `POST /api/v1/authored`, 200 posts per request, one request a second, tied to
`MeInfo::cache_key` so another member signing in wipes it). `GET /api/v1/replies?since=` catches up
after being away. Reply *push* is the one case where the server works out authorship itself, in
memory, for members who opted in (`member_reply_push`, `Database::reply_recipients`); see the design
constraint above for what that may do.

**Web push.** Nothing subscribes or prompts by itself. `pchanEnablePush` (in `index.html`, wrapped by
`frontend/src/push_client.rs`) runs from a click: starting to watch a thread, the "Push notifications
on this device" setting, or turning on reply push. After sign-in `pchanSyncPush` only re-tells the
server about a subscription the browser already has. The signing key is `VAPID_PRIVATE_KEY` in
`backend/.env`; the public key is derived from it and served at `GET /api/v1/push_key`, so the two
cannot disagree (rotating it invalidates every existing subscription). A subscription belongs to one browser: `member_push.endpoint_hash` (unique) lets a new
subscriber take it over, and the page unsubscribes on logout. Endpoints must be on a known push
service (`push_endpoint_allowed`), pushes are only forgotten on a 404/410, and the payload stays
under the ~3.8KB limit. `frontend/service_worker.js` skips the notification when the thread is
visible, tags one notification per thread (replies get their own), and "Go away." marks it read.

**Browser errors.** Crashes and uncaught errors are reported by `window.pchanReport` in
`frontend/index.html` (works even when the WASM is dead) to `POST /api/v1/client_errors`, deduplicated
into `logs/frontend-errors.json`, with older builds' entries moved to `...archive.json`. When someone
says "it broke", **read that file first**. It stores no identity and only to-the-minute first/last
times. Members can switch it off in settings.

Gotchas that each cost an afternoon:

- A server-sent event with **no `data:` line is never delivered** by browsers. Control events
  (`resync`, `inbox_changed`) carry a dummy body for that reason.
- The browser's own `open` event is a plain `Event`: reading `lastEventId` on it throws.
- WASM **cannot unwind**. A panic leaves whatever `RefCell` was borrowed locked, and the runtime then
  reports `RefCell already borrowed` on every later wake-up, far from the real panic. That is why
  listener callbacks each run as their own task, and why `main` installs a panic hook (the *first*
  `panic:` line in the console is the real one).
- Yew re-renders **synchronously** inside a state setter when called outside a render, so a callback
  that sets state can re-enter its own hook. Copy a callback out of a `RefCell` before calling it.
- `/api/v1/version` finds the build by the first `frontend_bg-` in `index.html`; text containing
  that string earlier in the page (a regex, a comment) used to break the update banner.

## Tests

```sh
cargo test                       # unit tests; the database ones are skipped
PCHAN_TEST_DATABASE_URL=postgres://postgres:disclone@localhost:5432/pchan_dev cargo test -p backend
```

The database tests (`backend/src/database_bindings/db_tests.rs`, helpers in `backend/src/testing.rs`)
write rows freely and take turns, so they never assume empty tables. They **refuse to run** unless
the database name ends in `_dev` or `_test`, so pointing them at `pchan_v2` is an error, not a
mistake. They cover the unread list and read marks, the authorship check, reply catch-up and
recipients, who hears about a post at each attention level, and push subscriptions. They do not
cover anything that needs a signed-in browser; those flows are tested by hand.

## Things that will mislead you

- **Status codes lie.** Handlers return **HTTP 200 with a JSON error string**, and a rejected
  static file falls through to the SPA shell as `200 text/html`. Assert on `content_type` and
  body, not on status. (Unauthenticated `/api/` requests are a real `401 application/json`.)
- **`pg_stat_user_tables.n_live_tup` is badly stale here** — it has reported 0 for tables
  holding hundreds of rows. Always `count(*)`.
- **`psql -c "a; b;"` only prints the last result.** Use separate `-c` flags.
- **`TRIM_TIME = 2`** means the file trimmer runs every two seconds and moves anything under
  `FILE_STORAGE_PATH` that is not a row in `files` to `TRASH_STORAGE_PATH`. Site assets under
  `/files/assets/` are excluded explicitly. Old uploads live in `trash/`, not deleted.
- **`warp::fs::dir` maps the full request path**, so URL `/files/x.jpg` resolves to
  `$FILE_STORAGE_PATH/files/x.jpg` — note the doubled segment.
- **Adding routes can break the build** with a recursion-depth error: every `.or()` nests
  another layer into one enormous filter type. `#![recursion_limit]` in `main.rs` is already
  raised; raise it further rather than boxing.
- Test data: use high ids and distinctive author seeds. The database holds real content.

## Design constraint

A member's posting history must not be linkable to their identity — by site mods, board mods,
or anyone holding a database dump. Keys live outside the database; `identities` and
`join_requests` hash the same Auth0 subject under **separate** keys so a dump cannot join them.
Before adding anything that logs, stores or returns a member identifier, check it against that.
The deliberate exception is `moderation_log`, documented in its migration.

**Decision (2026-10-19): the running server may work out who wrote a post, in memory.** The
requirement is that nobody can do it from the database alone, and that it takes deliberate work.
The keys live outside the database and the check needs a member's `author_seed`, so a dump
cannot do it; the running server necessarily can (deleting a post already proves authorship).
So a feature with a real need, such as pushing a reply to the author of the post replied to,
may compute authorship at send time. Rules for anything that does:

- Compute, use, discard. Never store, log or return a result that pairs a member with a post.
- No API may expose "who wrote this post". Only "did *I* write this" (`/api/v1/authored`).
- Features that act on it are opt-in, and the setting says what it tells third parties.
- A push still shows a push service (Google, Mozilla, Apple) the destination device and the
  time (the payload itself is encrypted to the device). The owner accepts contrived timing
  inferences of this kind, but wants any further relaxing of these rules proposed and
  discussed first. A reply push to the author therefore tells that vendor the device's owner wrote the
  post replied to. That is why reply pushes are opt-in and default off.
