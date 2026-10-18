//! Auth0 identity: verifying an ID token, and turning its subject into a member.
//!
//! The Auth0 subject (`sub`) is never stored. It is hashed under two separate keys — one for
//! `identities`, one for `join_requests` — so holding the database does not let anyone link a
//! pending join request to the member it eventually becomes.

use anyhow::{anyhow, bail, Context as _, Result};
use chrono::Utc;
use database::keys;
use deadpool::managed::Object;
use diesel::prelude::*;
use diesel_async::{pooled_connection::AsyncDieselConnectionManager, AsyncPgConnection, RunQueryDsl};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

type Conn = Object<AsyncDieselConnectionManager<AsyncPgConnection>>;

/// Cookie proving which Auth0 subject is filling in the join-request form. There is no session
/// yet at that point, so the subject has to survive one round trip.
const TICKET_COOKIE: &str = "join_ticket";
const TICKET_TTL_SECONDS: i64 = 1800;

/// What logging in with a given subject means.
pub enum SubjectOutcome {
    /// Known member; issue a session.
    Member(Uuid),
    /// Join request already submitted and awaiting review. Carries a ticket so the applicant
    /// can amend what they wrote while it sits in the queue.
    Pending(String),
    /// Declined recently, and the block has not expired.
    Blocked,
    /// Never seen. Carries the ticket authorising one join request.
    NeedsRequest(String),
}

pub async fn resolve_subject(conn: &mut Conn, subject: &str) -> Result<SubjectOutcome> {
    let identity = keys::identity_hash(subject);

    {
        use database::identities::dsl::*;
        if let Some(found) = identities
            .filter(identity_hash.eq(&identity))
            .select(member_id)
            .first::<Uuid>(conn)
            .await
            .optional()?
        {
            return Ok(SubjectOutcome::Member(found));
        }
    }

    // Bootstrap: on a site with no members at all, the first person to log in becomes the
    // admin. Without this there is no way to create the first account — the join queue needs
    // a reviewer, and the reviewer would need to already be a member.
    //
    // Self-limiting: it only ever fires against an empty members table. The risk it carries
    // is that if every member were deleted, the next person to log in would be made admin,
    // so a site being emptied deliberately should be re-seeded before being reachable.
    {
        use database::members::dsl::*;
        use diesel::dsl::{exists, select};
        let any_members: bool = select(exists(members.select(id))).get_result(conn).await?;
        if !any_members {
            log::warn!("no members exist; admitting the first login as the site admin");
            let member_id_value = create_member(conn, &identity, database::Role::Admin).await?;
            return Ok(SubjectOutcome::Member(member_id_value));
        }
    }

    // Different key, so this value cannot be correlated with the identity hash above.
    let request = keys::request_hash(subject);

    {
        use database::request_blocks::dsl::*;
        let blocked = request_blocks
            .filter(request_hash.eq(&request))
            .filter(until.gt(Utc::now()))
            .first::<(String, chrono::DateTime<Utc>)>(conn)
            .await
            .optional()?;
        if blocked.is_some() {
            return Ok(SubjectOutcome::Blocked);
        }
    }

    {
        use database::join_requests::dsl::*;
        let existing = join_requests
            .filter(request_hash.eq(&request))
            .select((id, approved_at))
            .first::<(Uuid, Option<chrono::DateTime<Utc>>)>(conn)
            .await
            .optional()?;

        match existing {
            // Approved and now claimed. This is the only moment the Auth0 subject and the
            // approval exist together, so it is the only moment the member can be created.
            Some((row_id, Some(_))) => {
                let member = create_member(conn, &identity, database::Role::Member).await?;
                // The disclosure text does not outlive the decision it informed.
                use database::join_requests::dsl as jr;
                diesel::delete(jr::join_requests.filter(jr::id.eq(row_id)))
                    .execute(conn)
                    .await?;
                return Ok(SubjectOutcome::Member(member));
            }
            Some((_, None)) => return Ok(SubjectOutcome::Pending(sign_ticket(&request))),
            None => {}
        }
    }

    Ok(SubjectOutcome::NeedsRequest(sign_ticket(&request)))
}

/// Create a member and bind an Auth0 identity to it, in one transaction.
///
/// The author seed is random and independent of the identity: nothing derivable from the
/// Auth0 subject ends up in `posts.actual_author`.
pub async fn create_member(
    conn: &mut Conn,
    identity: &str,
    role_value: database::Role,
) -> Result<Uuid> {
    use diesel_async::scoped_futures::ScopedFutureExt as _;
    use diesel_async::AsyncConnection as _;

    let identity = identity.to_owned();

    conn.transaction::<Uuid, anyhow::Error, _>(|conn| {
        async move {
            let seed = {
                let mut raw = [0u8; 32];
                rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut raw);
                hex::encode(raw)
            };

            let new_id = {
                use database::members::dsl::*;
                diesel::insert_into(members)
                    .values((author_seed.eq(&seed), role.eq(role_value.as_str())))
                    .returning(id)
                    .get_result::<Uuid>(conn)
                    .await?
            };

            {
                use database::identities::dsl::*;
                diesel::insert_into(identities)
                    .values((identity_hash.eq(&identity), member_id.eq(new_id)))
                    .execute(conn)
                    .await?;
            }

            Ok(new_id)
        }
        .scope_boxed()
    })
    .await
}

/// How long a declined applicant is blocked from reapplying.
///
/// Finite on purpose: a permanent row keyed on the subject would be a permanent pseudonym for
/// that person, which is the kind of durable record this schema exists to avoid.
const DECLINE_BLOCK_DAYS: i64 = 30;

/// Mark a pending request approved. The member is created when that person next logs in —
/// see [`resolve_subject`] for why it cannot happen here.
pub async fn approve_request(conn: &mut Conn, request_id: &Uuid) -> Result<bool> {
    use database::join_requests::dsl::*;
    let updated = diesel::update(
        join_requests
            .filter(id.eq(request_id))
            .filter(approved_at.is_null()),
    )
    .set(approved_at.eq(Utc::now()))
    .execute(conn)
    .await?;
    Ok(updated > 0)
}

/// Decline a request: drop it, and block reapplication for a while.
pub async fn decline_request(conn: &mut Conn, request_id: &Uuid) -> Result<bool> {
    use diesel_async::scoped_futures::ScopedFutureExt as _;
    use diesel_async::AsyncConnection as _;

    let request_id = *request_id;

    conn.transaction::<bool, anyhow::Error, _>(|conn| {
        async move {
            let hash = {
                use database::join_requests::dsl::*;
                join_requests
                    .filter(id.eq(request_id))
                    .select(request_hash)
                    .first::<String>(conn)
                    .await
                    .optional()?
            };

            let Some(hash) = hash else {
                return Ok(false);
            };

            {
                use database::request_blocks::dsl::*;
                diesel::insert_into(request_blocks)
                    .values((
                        request_hash.eq(&hash),
                        until.eq(Utc::now() + chrono::Duration::days(DECLINE_BLOCK_DAYS)),
                    ))
                    .on_conflict(request_hash)
                    .do_update()
                    .set(until.eq(Utc::now() + chrono::Duration::days(DECLINE_BLOCK_DAYS)))
                    .execute(conn)
                    .await?;
            }

            {
                use database::join_requests::dsl::*;
                diesel::delete(join_requests.filter(id.eq(request_id)))
                    .execute(conn)
                    .await?;
            }

            Ok(true)
        }
        .scope_boxed()
    })
    .await
}

/// The disclosure text already on file for this request, if any.
pub async fn get_request(conn: &mut Conn, request: &str) -> Result<Option<String>> {
    use database::join_requests::dsl::*;
    Ok(join_requests
        .filter(request_hash.eq(request))
        .select(disclosure)
        .first::<String>(conn)
        .await
        .optional()?)
}

/// Record a join request against the ticket issued at login.
pub async fn submit_request(conn: &mut Conn, request: &str, disclosure_text: &str) -> Result<()> {
    use database::join_requests::dsl::*;
    diesel::insert_into(join_requests)
        .values((request_hash.eq(request), disclosure.eq(disclosure_text)))
        // Re-submitting replaces the text rather than queueing a second row.
        .on_conflict(request_hash)
        .do_update()
        .set(disclosure.eq(disclosure_text))
        .execute(conn)
        .await?;
    Ok(())
}

/// Pending requests, oldest first.
///
/// Returns only the id, the applicant's own words, and the time. Never `request_hash` — a
/// reviewer who recorded that could look for it later and link a person to the member they
/// became.
pub async fn list_pending_requests(
    conn: &mut Conn,
) -> Result<Vec<common::structs::JoinRequestSummary>> {
    use database::join_requests::dsl::*;

    let rows = join_requests
        .filter(approved_at.is_null())
        .order(created_at.asc())
        .select((id, disclosure, created_at))
        .load::<(Uuid, String, chrono::DateTime<Utc>)>(conn)
        .await?;

    Ok(rows
        .into_iter()
        .map(|(row_id, text, at)| common::structs::JoinRequestSummary {
            id: row_id.to_string(),
            disclosure: text,
            created_at: at,
        })
        .collect())
}

/// Ban the author of a post without telling the caller who that is.
///
/// Moderation is expressed against posts, never against members: a mod acts on something they
/// can see, and the server does the resolution. `actual_author` is
/// `HMAC(POST_AUTHOR_KEY, author_seed ‖ post_id)`, so finding the author means recomputing it
/// for each member — one HMAC per member, which is nothing at this scale and is the price of
/// the table not storing who wrote what.
///
/// Returns whether an author was found. The caller is told only that, never the identity.
pub async fn ban_post_author(
    conn: &mut Conn,
    discriminator: &str,
    post_number: i64,
    viewer: &database::Viewer,
) -> Result<bool> {
    // Addressed the way every other post endpoint is, because board + post number is what a
    // client actually has; the internal row id is never exposed. get_raw_post is reached via
    // get_board so a mod cannot act on a board they cannot see.
    let target_board =
        crate::database_bindings::Database::get_board(conn, discriminator, viewer).await?;

    let (post_id, target) = {
        use database::posts::dsl as p;
        p::posts
            .filter(p::board.eq(target_board.id))
            .filter(p::post_number.eq(post_number))
            .select((p::id, p::actual_author))
            .first::<(i64, String)>(conn)
            .await
            .optional()?
            .ok_or_else(|| anyhow!("no such post"))?
    };

    let candidates = {
        use database::members::dsl::*;
        members
            .filter(banned.eq(false))
            .select((id, author_seed))
            .load::<(Uuid, String)>(conn)
            .await?
    };

    let found = candidates
        .into_iter()
        .find(|(_, seed)| keys::post_author_hash(seed, post_id) == target);

    let Some((member, _)) = found else {
        // The author rotated their seed, or their account is gone. Nothing to ban.
        return Ok(false);
    };

    {
        use database::members::dsl::*;
        diesel::update(members.filter(id.eq(member)))
            .set(banned.eq(true))
            .execute(conn)
            .await?;
    }

    // Revoke their sessions so the ban is immediate rather than lasting until expiry.
    crate::session::destroy_all_for_member(conn, &member).await?;
    crate::filters::forget_cached_member(&member).await;

    Ok(true)
}

/// Replace a member's author seed, severing them from everything they have posted.
///
/// The cost is deliberate and must be surfaced in the UI: they can no longer delete their own
/// earlier posts, because `actual_author` no longer matches.
pub async fn rotate_author_seed(conn: &mut Conn, member: &Uuid) -> Result<()> {
    use database::members::dsl::*;

    let seed = {
        let mut raw = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut raw);
        hex::encode(raw)
    };

    diesel::update(members.filter(id.eq(member)))
        .set(author_seed.eq(seed))
        .execute(conn)
        .await?;

    // The cached viewer still holds the old seed.
    crate::filters::forget_cached_member(member).await;
    Ok(())
}

// ---------------------------------------------------------------- ticket

/// Sign the join ticket so a client cannot mint one for an arbitrary request hash.
fn sign_ticket(request_hash: &str) -> String {
    let expiry = Utc::now().timestamp() + TICKET_TTL_SECONDS;
    let mac = keys::ticket_mac(request_hash, expiry);
    format!("{request_hash}.{expiry}.{mac}")
}

/// Verify a join ticket and return the request hash it authorises.
pub fn verify_ticket(ticket: &str) -> Result<String> {
    let mut parts = ticket.split('.');
    let (request_hash, expiry, mac) = match (parts.next(), parts.next(), parts.next()) {
        (Some(a), Some(b), Some(c)) if parts.next().is_none() => (a, b, c),
        _ => bail!("malformed join ticket"),
    };

    let expiry: i64 = expiry.parse().context("join ticket had a bad expiry")?;
    if Utc::now().timestamp() > expiry {
        bail!("join ticket has expired");
    }

    let expected = keys::ticket_mac(request_hash, expiry);
    // Both sides are hex of a fixed length; compare without an early exit.
    if !constant_time_eq(expected.as_bytes(), mac.as_bytes()) {
        bail!("join ticket signature did not verify");
    }

    Ok(request_hash.to_owned())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn ticket_cookie_header(ticket: &str) -> String {
    let secure = if crate::config::get().cookie_secure {
        "; Secure"
    } else {
        ""
    };
    format!(
        "{TICKET_COOKIE}={ticket}; Path=/; HttpOnly; SameSite=Lax{secure}; Max-Age={TICKET_TTL_SECONDS}"
    )
}

pub fn ticket_cookie_name() -> &'static str {
    TICKET_COOKIE
}

// ---------------------------------------------------------------- id token

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
    nonce: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct Jwk {
    kid: String,
    n: String,
    e: String,
}

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

lazy_static::lazy_static! {
    static ref JWKS_CACHE: Arc<RwLock<Option<(std::time::Instant, HashMap<String, Jwk>)>>> =
        Arc::new(RwLock::new(None));
}

const JWKS_TTL: std::time::Duration = std::time::Duration::from_secs(3600);

async fn jwk_for(kid: &str) -> Result<Jwk> {
    if let Some((fetched, keys)) = JWKS_CACHE.read().await.as_ref() {
        if fetched.elapsed() < JWKS_TTL {
            if let Some(k) = keys.get(kid) {
                return Ok(k.clone());
            }
        }
    }

    // Either the cache is cold or the key is unknown — Auth0 rotates signing keys, so an
    // unrecognised kid means refetch rather than reject.
    let cfg = crate::config::get();
    let fetched: Jwks = reqwest::get(cfg.auth0.jwks_url())
        .await
        .context("could not fetch Auth0 JWKS")?
        .json()
        .await
        .context("Auth0 JWKS was not valid JSON")?;

    let map: HashMap<String, Jwk> = fetched
        .keys
        .into_iter()
        .map(|k| (k.kid.clone(), k))
        .collect();

    let found = map.get(kid).cloned();
    *JWKS_CACHE.write().await = Some((std::time::Instant::now(), map));

    found.ok_or_else(|| anyhow!("Auth0 JWKS has no key with kid {kid}"))
}

/// Verify an ID token's signature and claims, returning the subject.
///
/// Checks the RS256 signature against the tenant's published key, plus issuer, audience,
/// expiry, and the nonce bound to this particular login attempt.
pub async fn verify_id_token(id_token: &str, expected_nonce: &str) -> Result<String> {
    use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};

    let cfg = crate::config::get();

    let header = decode_header(id_token).context("ID token header was unreadable")?;
    let kid = header.kid.context("ID token had no kid")?;
    if header.alg != Algorithm::RS256 {
        bail!("unexpected ID token algorithm {:?}", header.alg);
    }

    let jwk = jwk_for(&kid).await?;
    let key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
        .context("Auth0 signing key was not usable")?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[cfg.auth0.client_id.as_str()]);
    validation.set_issuer(&[cfg.auth0.issuer()]);
    validation.validate_exp = true;

    let token = decode::<Claims>(id_token, &key, &validation)
        .context("ID token failed verification")?;

    // Binds the token to the login this browser actually started, which is what stops a token
    // obtained elsewhere from being replayed into someone else's flow.
    match token.claims.nonce.as_deref() {
        Some(n) if n == expected_nonce => {}
        Some(_) => bail!("ID token nonce did not match"),
        None => bail!("ID token had no nonce"),
    }

    if token.claims.sub.is_empty() {
        bail!("ID token had an empty subject");
    }

    Ok(token.claims.sub)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyring() {
        // Independent values; the real ones come from the environment.
        let _ = database::keys::init(database::keys::Keyring {
            identity: vec![11; 32],
            request: vec![22; 32],
            post_author: vec![33; 32],
            invite: vec![44; 32],
            push_encryption: vec![55; 32],
        });
    }

    #[test]
    fn ticket_round_trips() {
        keyring();
        let request = keys::request_hash("oauth2|discord|123");
        let ticket = sign_ticket(&request);
        assert_eq!(verify_ticket(&ticket).expect("valid ticket"), request);
    }

    #[test]
    fn ticket_rejects_tampering() {
        keyring();
        let request = keys::request_hash("oauth2|discord|123");
        let ticket = sign_ticket(&request);

        // Swapping in someone else's request hash must not verify: otherwise anyone could
        // file a join request against an arbitrary identity.
        let other = keys::request_hash("oauth2|discord|999");
        let parts: Vec<&str> = ticket.split('.').collect();
        let forged = format!("{}.{}.{}", other, parts[1], parts[2]);
        assert!(verify_ticket(&forged).is_err(), "forged hash must be refused");

        // Extending the expiry must not verify either.
        let far_future = Utc::now().timestamp() + 999_999;
        let extended = format!("{}.{}.{}", parts[0], far_future, parts[2]);
        assert!(verify_ticket(&extended).is_err(), "extended expiry must be refused");

        assert!(verify_ticket("nonsense").is_err());
        assert!(verify_ticket("a.b.c").is_err());
    }

    #[test]
    fn ticket_rejects_expired() {
        keyring();
        let request = keys::request_hash("oauth2|discord|123");
        let expiry = Utc::now().timestamp() - 1;
        let mac = keys::ticket_mac(&request, expiry);
        // Correctly signed but stale: the signature is valid, the window is not.
        let stale = format!("{request}.{expiry}.{mac}");
        assert!(verify_ticket(&stale).is_err(), "expired ticket must be refused");
    }
}
