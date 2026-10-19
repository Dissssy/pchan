//! Support for tests that need a real Postgres.
//!
//! These tests are destructive and write rows freely, so they only run when
//! `PCHAN_TEST_DATABASE_URL` names a database whose name ends in `_dev` or `_test`. With it unset
//! they are skipped (loudly), so a plain `cargo test` keeps working anywhere. Never point it at
//! `pchan_v2`: the name check exists to make that a refusal, not a mistake.
//!
//! ```sh
//! PCHAN_TEST_DATABASE_URL=postgres://postgres:disclone@localhost:5432/pchan_dev \
//!   cargo test -p backend
//! ```
//!
//! The database must already be migrated (`diesel migration run`). Tests share it, so each makes
//! its own board and members with random names and never assumes the tables are empty.

use std::sync::{Once, OnceLock};

use deadpool::managed::Object;
use diesel::prelude::*;
use diesel_async::{pooled_connection::AsyncDieselConnectionManager, AsyncPgConnection, RunQueryDsl};
use uuid::Uuid;

pub type Conn = Object<AsyncDieselConnectionManager<AsyncPgConnection>>;

static INIT: Once = Once::new();

/// Tests that touch the database take turns: some look at tables as a whole (the newest post, a
/// page of replies), which other tests adding rows at the same time would disturb.
static TURN: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

pub struct Db {
    pub conn: Conn,
    _turn: tokio::sync::MutexGuard<'static, ()>,
}

/// A connection to the test database, or `None` if tests that need one should be skipped.
pub async fn db() -> Option<Db> {
    let Ok(url) = std::env::var("PCHAN_TEST_DATABASE_URL") else {
        eprintln!("SKIPPED: set PCHAN_TEST_DATABASE_URL to a *_dev or *_test database to run this");
        return None;
    };
    let name = url.rsplit('/').next().unwrap_or("").split('?').next().unwrap_or("");
    assert!(
        name.ends_with("_dev") || name.ends_with("_test"),
        "refusing to run destructive tests against `{name}`: the database name must end in _dev or _test"
    );

    INIT.call_once(|| {
        // The pool is created from DATABASE_URL on first use, so this has to come first.
        std::env::set_var("DATABASE_URL", &url);
        // Not the real keys: these only have to be present and distinct.
        let _ = database::keys::init(database::keys::Keyring {
            identity: vec![1; 32],
            request: vec![2; 32],
            post_author: vec![3; 32],
            invite: vec![4; 32],
            push_encryption: vec![5; 32],
        });
    });

    let turn = TURN
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let conn = crate::POOL.get().await.expect("could not connect to the test database");
    Some(Db { conn, _turn: turn })
}

fn tag() -> String {
    Uuid::new_v4().simple().to_string()[..10].to_owned()
}

/// A new ordinary member with a random posting identity.
pub async fn member(conn: &mut Conn) -> database::Member {
    use database::members::dsl::*;
    diesel::insert_into(members)
        .values((
            id.eq(Uuid::new_v4()),
            author_seed.eq(tag() + &tag()),
            role.eq("member"),
        ))
        .get_result::<database::Member>(conn)
        .await
        .expect("insert member")
}

pub fn viewer(member: &database::Member) -> database::Viewer {
    member.viewer_with(database::Ban::None)
}

/// A new board with a random name.
pub async fn board(conn: &mut Conn, make_private: bool) -> database::Board {
    use database::boards::dsl::*;
    let t = tag();
    diesel::insert_into(boards)
        .values((name.eq(format!("Test {t}")), discriminator.eq(format!("t{t}")), private.eq(make_private)))
        .get_result::<database::Board>(conn)
        .await
        .expect("insert board")
}

/// Let `member` see a private board.
pub async fn grant_access(conn: &mut Conn, on: &database::Board, who: &database::Member) {
    use database::user_tags::dsl::*;
    diesel::insert_into(user_tags)
        .values((
            id.eq(Uuid::new_v4()),
            board_id.eq(on.id),
            user_name.eq("test"),
            invite_hash.eq(Some(viewer(who).invite_hash(on.id))),
            tag_kind.eq(database::UserTag::BoardAccess.to_string()),
            generated_by.eq("ADMIN"),
        ))
        .execute(conn)
        .await
        .expect("grant access");
}

/// Take it back.
pub async fn revoke_access(conn: &mut Conn, on: &database::Board, who: &database::Member) {
    use database::user_tags::dsl::*;
    diesel::delete(
        user_tags
            .filter(board_id.eq(on.id))
            .filter(invite_hash.eq(Some(viewer(who).invite_hash(on.id)))),
    )
    .execute(conn)
    .await
    .expect("revoke access");
}

/// A post by `by`, with `reply_to` the ids of the posts it replies to. Written straight to the
/// table, so none of the app's checks or side effects (uploads, notifications) happen.
pub async fn post(
    conn: &mut Conn,
    on: &database::Board,
    in_thread: &database::Thread,
    by: &database::Member,
    text: &str,
    reply_to: Vec<i64>,
) -> database::Post {
    use database::posts::dsl as p;

    let number: i64 = {
        use database::boards::dsl::*;
        boards.filter(id.eq(on.id)).select(post_count).first::<i64>(conn).await.expect("board") + 1
    };
    let new_id: i64 = diesel::select(diesel::dsl::sql::<diesel::sql_types::BigInt>("nextval('posts_id_seq')"))
        .get_result(conn)
        .await
        .expect("post id");
    diesel::insert_into(p::posts)
        .values((
            p::id.eq(new_id),
            p::post_number.eq(number),
            p::thread.eq(in_thread.id),
            p::board.eq(on.id),
            p::author.eq(None::<String>),
            p::content.eq(text),
            p::replies_to.eq(reply_to),
            p::timestamp.eq(chrono::Utc::now().naive_utc()),
            p::actual_author.eq(viewer(by).post_author_hash(new_id)),
            p::moderator.eq(false),
        ))
        .get_result::<database::Post>(conn)
        .await
        .expect("insert post")
}

/// A thread with its first post.
pub async fn thread(
    conn: &mut Conn,
    on: &database::Board,
    by: &database::Member,
    topic: &str,
) -> (database::Thread, database::Post) {
    use database::threads::dsl as t;
    let created = diesel::insert_into(t::threads)
        .values((t::board.eq(on.id), t::post_id.eq(0), t::latest_post.eq(0), t::topic.eq(topic)))
        .get_result::<database::Thread>(conn)
        .await
        .expect("insert thread");
    let op = post(conn, on, &created, by, "opening post", vec![]).await;
    // A trigger points the thread at its first post; read it back.
    let created = t::threads
        .filter(t::id.eq(created.id))
        .first::<database::Thread>(conn)
        .await
        .expect("reread thread");
    (created, op)
}
