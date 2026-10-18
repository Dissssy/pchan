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
    /// Join request already submitted and awaiting review.
    Pending,
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
        let pending = join_requests
            .filter(request_hash.eq(&request))
            .select(id)
            .first::<Uuid>(conn)
            .await
            .optional()?;
        if pending.is_some() {
            return Ok(SubjectOutcome::Pending);
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

/// Approve a pending request: create the member and bind the identity, then delete the
/// request so the disclosure text does not outlive the decision.
///
/// `subject_identity_hash` has to be supplied because the request row cannot be linked back to
/// an identity by design — the member is only created when that person next logs in.
pub async fn approve_request(conn: &mut Conn, request: &str) -> Result<()> {
    use database::join_requests::dsl as jr;
    diesel::delete(jr::join_requests.filter(jr::request_hash.eq(request)))
        .execute(conn)
        .await?;
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
