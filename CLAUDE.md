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
  `PUBLIC_URL`, `COOKIE_SECURE`. Loaded at startup; validation refuses to boot on a missing,
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
- `pchan_dev` — an old copy of live. Not scratch space.

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
