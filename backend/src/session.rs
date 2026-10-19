//! Server-side sessions.
//!
//! The previous scheme used a cookie that was a pure function of the user's Discord id
//! (`sha256(id ‖ salt)`), with a ten year lifetime. That made it an unrevocable bearer
//! credential: it could not be rotated without changing the salt for everyone, logging out was
//! impossible, and `GET /api/v1/token` deliberately handed it to JavaScript.
//!
//! A session is now 32 bytes from the OS CSPRNG, stored only as a hash, with an expiry and a
//! row that can be deleted. Logging out, revoking one device, and banning all actually work.

use anyhow::Result;
use chrono::{Duration, Utc};
use database::Viewer;
use deadpool::managed::Object;
use diesel::prelude::*;
use diesel_async::{pooled_connection::AsyncDieselConnectionManager, AsyncPgConnection, RunQueryDsl};
use rand::RngCore as _;
use uuid::Uuid;

pub const COOKIE_NAME: &str = "session";

/// Absolute lifetime. There is no refresh: logging in again is cheap, and a bounded lifetime
/// means a leaked cookie stops working on its own.
const SESSION_LIFETIME_DAYS: i64 = 30;

type Conn = Object<AsyncDieselConnectionManager<AsyncPgConnection>>;

/// Mint a session and return the raw cookie value. This is the only time the raw value
/// exists; only its hash is stored.
pub async fn create(conn: &mut Conn, member: &Uuid) -> Result<String> {
    use database::sessions::dsl::*;

    let mut raw = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw);
    let token = hex::encode(raw);

    diesel::insert_into(sessions)
        .values((
            token_hash.eq(database::keys::session_token_hash(&token)),
            member_id.eq(member),
            expires_at.eq(Utc::now() + Duration::days(SESSION_LIFETIME_DAYS)),
        ))
        .execute(conn)
        .await?;

    Ok(token)
}

/// Resolve a cookie value to the member behind it.
///
/// Returns `None` for an unknown, expired, or banned session rather than an error, so callers
/// cannot accidentally treat a lookup failure as authentication.
pub async fn resolve(conn: &mut Conn, token: &str) -> Result<Option<Viewer>> {
    use database::members::dsl as m;
    use database::sessions::dsl as s;

    let hashed = database::keys::session_token_hash(token);

    let found = s::sessions
        .inner_join(m::members.on(m::id.eq(s::member_id)))
        .filter(s::token_hash.eq(&hashed))
        .filter(s::expires_at.gt(Utc::now()))
        .select((m::id, m::author_seed, m::role))
        .first::<(Uuid, String, String)>(conn)
        .await
        .optional()?;

    let Some((id, author_seed, role)) = found else {
        return Ok(None);
    };

    // Bans live in their own table now, so this is a second small query. It sits behind the
    // viewer cache, so it runs about once per cache TTL per session rather than per request.
    let ban = database::resolve_ban(conn, &id).await?;

    // A sitewide ban means no session at all, so it takes effect without hunting down the
    // member's rows. A board-scoped ban still lets them sign in; check_access refuses the
    // boards it covers.
    if ban.is_sitewide() {
        return Ok(None);
    }

    Ok(Some(Viewer {
        id,
        author_seed,
        role: database::Role::from_str(&role),
        ban,
    }))
}

/// Record that a session was used. Best-effort: a failure here must not fail the request.
pub async fn touch(conn: &mut Conn, token: &str) {
    use database::sessions::dsl::*;
    let hashed = database::keys::session_token_hash(token);
    if let Err(e) = diesel::update(sessions.filter(token_hash.eq(&hashed)))
        .set(last_seen.eq(Utc::now()))
        .execute(conn)
        .await
    {
        log::warn!("could not update session last_seen: {e}");
    }
}

/// Log out one session.
pub async fn destroy(conn: &mut Conn, token: &str) -> Result<()> {
    use database::sessions::dsl::*;
    let hashed = database::keys::session_token_hash(token);
    diesel::delete(sessions.filter(token_hash.eq(&hashed)))
        .execute(conn)
        .await?;
    Ok(())
}

/// Log out everywhere. Used when banning and when a member rotates their identity.
pub async fn destroy_all_for_member(conn: &mut Conn, member: &Uuid) -> Result<()> {
    use database::sessions::dsl::*;
    diesel::delete(sessions.filter(member_id.eq(member)))
        .execute(conn)
        .await?;
    Ok(())
}

/// Delete expired rows. Nothing reads them, but they are a growing record of when people
/// were active.
pub async fn purge_expired(conn: &mut Conn) -> Result<usize> {
    use database::sessions::dsl::*;
    Ok(diesel::delete(sessions.filter(expires_at.lt(Utc::now())))
        .execute(conn)
        .await?)
}

/// The `Set-Cookie` value for a new session.
pub fn cookie_header(token: &str) -> String {
    let secure = if crate::config::get().cookie_secure {
        "; Secure"
    } else {
        ""
    };
    // SameSite=Lax, not Strict: board invite links are opened from Discord as a cross-site
    // top-level navigation, and Strict would strip the cookie and bounce the user to login.
    format!(
        "{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Lax{secure}; Max-Age={}",
        SESSION_LIFETIME_DAYS * 24 * 60 * 60
    )
}

/// The `Set-Cookie` value that clears the session cookie.
pub fn clear_cookie_header() -> String {
    let secure = if crate::config::get().cookie_secure {
        "; Secure"
    } else {
        ""
    };
    format!("{COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax{secure}; Max-Age=0")
}
