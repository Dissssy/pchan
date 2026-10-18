use base64::Engine as _;
use common::hash_with_salt;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::Display,
    hash::{Hash, Hasher},
    str::FromStr,
    sync::Arc,
};
use tokio::{sync::Mutex, time::Instant};
use warp::{path::FullPath, Filter};

#[cfg(not(feature = "no_ratelimit"))]
use warp::http::Method;

lazy_static! {
    /// Short-lived cache of resolved sessions, keyed on the hashed cookie so the raw
    /// credential is not a key in process memory.
    static ref SESSION_CACHE: Arc<Mutex<HashMap<String, (Instant, database::Viewer)>>> =
        Arc::new(Mutex::new(HashMap::new()));
}

/// How long a resolved session is reused before being re-read from the database. This is the
/// window in which a logout or ban has not taken effect yet, so it is deliberately short.
const SESSION_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(5);

/// The authenticated member, or a rejection.
pub fn viewer() -> impl Filter<Extract = (database::Viewer,), Error = warp::Rejection> + Clone {
    warp::any()
        .and(optional_viewer())
        .and_then(|v: Option<database::Viewer>| async move {
            match v {
                Some(v) => Ok(v),
                None => Err(warp::reject::reject()),
            }
        })
}

/// Serves the frontend shell to everyone, so a logged-out visitor can reach the login and
/// request-access pages. Static assets under /res/ stay open for the same reason.
pub fn allow_public_shell() -> impl Filter<Extract = ((),), Error = warp::Rejection> + Clone {
    warp::any().and_then(|| async { Ok::<_, warp::Rejection>(()) })
}

/// Resolve the session cookie to a member, if there is one.
///
/// Unlike the old filter there is no `Authorization` fallback: the browser is the only client,
/// and `GET /api/v1/token` no longer exists to hand a credential to JavaScript.
pub fn optional_viewer(
) -> impl Filter<Extract = (Option<database::Viewer>,), Error = warp::Rejection> + Clone {
    warp::any()
        .and(warp::cookie::optional(crate::session::COOKIE_NAME))
        .and_then(|cookie: Option<String>| async move {
            let Some(token) = cookie else {
                return Ok(None);
            };

            let cache_key = database::keys::session_token_hash(&token);

            {
                let mut cache = SESSION_CACHE.lock().await;
                if let Entry::Occupied(e) = cache.entry(cache_key.clone()) {
                    if e.get().0.elapsed() < SESSION_CACHE_TTL {
                        return Ok(Some(e.get().1.clone()));
                    }
                    e.remove();
                }
            }

            // Lock released before the query: holding it across the round trip serialised
            // every authenticated request behind one database call.
            let mut conn = crate::POOL.get().await.map_err(|e| {
                log::error!("could not get a connection to resolve a session: {e}");
                warp::reject::reject()
            })?;

            let resolved = crate::session::resolve(&mut conn, &token)
                .await
                .map_err(|e| {
                    // Logged rather than swallowed: a schema or query fault here is
                    // indistinguishable from "not logged in" to the caller, which is
                    // miserable to debug.
                    log::error!("error resolving session: {e}");
                    warp::reject::reject()
                })?;

            if let Some(v) = &resolved {
                SESSION_CACHE
                    .lock()
                    .await
                    .insert(cache_key, (Instant::now(), v.clone()));
            }

            Ok::<_, warp::reject::Rejection>(resolved)
        })
}

/// Drop a session from the resolve cache so a logout or ban takes effect immediately rather
/// than at the end of the TTL.
pub async fn forget_cached_session(token: &str) {
    SESSION_CACHE
        .lock()
        .await
        .remove(&database::keys::session_token_hash(token));
}

/// Drop every cached session for one member. Used when banning.
pub async fn forget_cached_member(member: &uuid::Uuid) {
    SESSION_CACHE.lock().await.retain(|_, (_, v)| &v.id != member);
}

pub fn optional_file_sig(
) -> impl Filter<Extract = (Option<FileSig>,), Error = warp::Rejection> + Clone {
    // check for FileSig query parameters
    warp::any()
        .and(warp::query::query::<MaybeFileSig>())
        .map(|file_sig: MaybeFileSig| {
            if let MaybeFileSig {
                bgn: Some(bgn),
                exp: Some(exp),
                sig: Some(sig),
            } = file_sig
            {
                Some(FileSig { bgn, exp, sig })
            } else {
                None
            }
        })
}

pub fn optional_file(
) -> impl Filter<Extract = (Option<warp::fs::File>,), Error = warp::Rejection> + Clone {
    warp::any()
        .and(warp::fs::dir(env!("FILE_STORAGE_PATH")))
        .map(|file: warp::fs::File| Some(file))
        .recover(|_| async { Ok::<_, warp::Rejection>(None::<warp::fs::File>) })
        .unify()
}

pub struct FileSig {
    pub bgn: u64,
    pub exp: u64,
    pub sig: String,
}

impl FileSig {
    pub async fn validates(&self, path: &str) -> bool {
        let Self { bgn, exp, sig } = self;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        if now < *bgn {
            return false;
        }

        if now > *exp {
            return false;
        }

        let val = format!("{}|{}|{}", path, bgn, exp);

        let new_sig = crate::statics::BASE64_ENGINE.encode(hmac_sha512::HMAC::mac(
            val,
            crate::statics::HMAC_KEY_GENERATOR.get_key().await,
        ));

        new_sig == *sig
    }

    #[allow(
        clippy::panic,
        reason = "Panics only if the generated FileSig fails to validate, which should never happen"
    )]
    pub async fn generate(path: &str) -> Self {
        let bgn = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
        let exp = bgn + crate::statics::FILE_SHARE_DURATION;
        let val = format!("{}|{}|{}", path, bgn, exp);

        let sig = crate::statics::BASE64_ENGINE.encode(hmac_sha512::HMAC::mac(
            val,
            crate::statics::HMAC_KEY_GENERATOR.get_key().await,
        ));

        let s = Self { bgn, exp, sig };

        if s.validates(path).await {
            s
        } else {
            panic!("Failed to validate generated FileSig");
        }
    }
}

#[derive(Deserialize)]
pub struct MaybeFileSig {
    pub bgn: Option<u64>,
    pub exp: Option<u64>,
    pub sig: Option<String>,
}

pub fn priveleged_endpoint() -> impl Filter<Extract = (), Error = warp::Rejection> + Clone {
    warp::header::header::<Bearer>("authorization")
        .and_then(|header: Bearer| async move {
            if header.token != *crate::statics::TOKEN {
                Err(warp::reject::reject())
            } else {
                Ok(())
            }
        })
        .and(warp::any())
        .untuple_one()
}

pub fn user_agent_is_scraper() -> impl Filter<Extract = (), Error = warp::Rejection> + Clone {
    warp::header::header::<String>("user-agent")
        .and_then(|user_agent: String| async move {
            log::trace!("User agent: {}", user_agent);
            if crate::statics::KNOWN_SCRAPERS.contains(&user_agent.as_str()) {
                Ok(())
            } else {
                Err(warp::reject::reject())
            }
        })
        .and(warp::any())
        .untuple_one()
}

#[cfg_attr(feature = "no_ratelimit", allow(unused_variables))]
pub fn ratelimit() -> impl Filter<Extract = (), Error = warp::Rejection> + Clone {
    warp::path::full()
        .and(viewer())
        .and(warp::method())
        .and_then(|path: FullPath, viewer: database::Viewer, method| async move {
            #[cfg(feature = "no_ratelimit")]
            {
                Ok::<(), warp::Rejection>(())
            }
            #[cfg(not(feature = "no_ratelimit"))]
            {
                if method == Method::GET {
                    return Ok(());
                }

                // The role rides on the viewer now, so this no longer costs a query on
                // every mutating request.
                if viewer.role.is_admin() {
                    return Ok(());
                }

                // is formatted as after the url
                let rawpath = path.as_str();
                let mut path = rawpath.split('/').skip(3);
                let path = [path.next(), path.next(), path.next(), path.next()];

                // POST /board/?/thread/?
                // DELETE /board/?/post/?
                // POST /board/?/thread
                // POST /subscribe
                // POST /file
                let include_thread = std::env::var("THREAD_SPECIFIC_RATELIMIT")
                    .map(|v| v.parse::<bool>().unwrap_or_default())
                    .unwrap_or_default();
                let include_board = include_thread
                    || std::env::var("BOARD_SPECIFIC_RATELIMIT")
                        .map(|v| v.parse::<bool>().unwrap_or_default())
                        .unwrap_or_default();

                let (seconds, ident): (u64, Arc<str>) =
                    match (method, path[0], path[1], path[2], path[3]) {
                        (
                            Method::POST,
                            Some("board"),
                            Some(discrim),
                            Some("thread"),
                            Some(thread),
                        ) => (
                            5,
                            format!(
                                "make post{}{}",
                                if include_board {
                                    format!(" on /{}/", discrim)
                                } else {
                                    String::new()
                                },
                                if include_thread {
                                    format!(" in {}", thread)
                                } else {
                                    String::new()
                                }
                            )
                            .into(),
                        ),
                        (
                            Method::DELETE,
                            Some("board"),
                            Some(discrim),
                            Some("post"),
                            Some(post),
                        ) => (
                            5,
                            format!(
                                "delete post{}{}",
                                if include_board {
                                    format!(" on /{}/", discrim)
                                } else {
                                    String::new()
                                },
                                if include_thread {
                                    format!(" in {}", post)
                                } else {
                                    String::new()
                                }
                            )
                            .into(),
                        ),
                        (Method::POST, Some("board"), Some(discrim), Some("thread"), None) => (
                            10,
                            format!(
                                "make thread{}",
                                if include_board {
                                    format!(" on /{}/", discrim)
                                } else {
                                    String::new()
                                }
                            )
                            .into(),
                        ),
                        (Method::POST, Some("subscribe"), _, _, _) => (3, "subscribe".into()),
                        (Method::POST, Some("file"), _, _, _) => (15, "file upload".into()),
                        (Method::PUT, _, _, _, _) => (0, "PUT".into()),
                        (Method::PATCH, Some("consume_code"), None, None, None) => {
                            (60, "consume code".into())
                        }
                        path => {
                            log::info!("path: {:?}", path);
                            (5, rawpath.into())
                        }
                    };

                // Keyed on the member hash, not the raw credential, so the bearer token is
                // not sitting in process memory as a map key.
                let total_string = format!("{}|{}", ident, viewer.id);
                let mut ratelimit = crate::RATELIMIT.lock().await;
                match ratelimit.entry(total_string) {
                    Entry::Occupied(mut entry) => {
                        // the entry exists, ensure the time it is set to has elapsed
                        let t = entry.get();
                        if t < &tokio::time::Instant::now() {
                            // the time has elapsed, update the time to now + seconds
                            entry.insert(
                                tokio::time::Instant::now()
                                    + tokio::time::Duration::from_secs(seconds),
                            );
                        } else {
                            // the time has not elapsed, return an error
                            return Err(warp::reject::custom(Ratelimited {
                                seconds: t.duration_since(tokio::time::Instant::now()).as_secs(),
                            }));
                        }
                    }
                    Entry::Vacant(entry) => {
                        // the entry does not exist, create it and set the time to now + seconds
                        entry.insert(
                            tokio::time::Instant::now() + tokio::time::Duration::from_secs(seconds),
                        );
                    }
                }
                Ok(())
            }
        })
        .untuple_one()
}

#[derive(Debug)]
pub struct Ratelimited {
    pub seconds: u64,
}

impl warp::reject::Reject for Ratelimited {}

// pub fn is_beta() -> impl Filter<Extract = (), Error = warp::Rejection> + Clone {
//     warp::cookie::cookie::<bool>("beta")
//         .and_then(|beta: bool| async move {
//             if beta {
//                 Ok(())
//             } else {
//                 Err(warp::reject::reject())
//             }
//         })
//         .and(warp::any())
//         .untuple_one()
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bearer {
    pub token: String,
}

impl FromStr for Bearer {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s
            .split("Bearer ")
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("Invalid Bearer token"))?;
        Ok(Self {
            token: s.to_string(),
        })
    }
}

impl Display for Bearer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bearer {}", self.token)
    }
}

#[derive(Clone)]
pub struct Token {
    token: Arc<String>,
    cached_member_hash: Option<MemberToken>,
}

// Display and Debug are deliberately redacted. This value is a bearer credential, and log
// access has to be treated as equivalent to deanonymizing every member. Use cookie_value()
// at the one place that legitimately needs the raw string.
impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(redacted)")
    }
}

impl PartialEq for Token {
    fn eq(&self, other: &Self) -> bool {
        self.token == other.token
    }
}

impl Eq for Token {}

impl Hash for Token {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.token.hash(state);
    }
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(redacted)")
    }
}

impl Token {
    pub fn new(token: String) -> Self {
        Self {
            token: Arc::new(token),
            cached_member_hash: None,
        }
    }
    pub fn member_hash(&mut self) -> MemberToken {
        if let Some(member_hash) = &self.cached_member_hash {
            return member_hash.clone();
        }
        let m = MemberToken::new(Arc::new(hash_with_salt(
            &self.token,
            &crate::statics::TOKEN_SALT,
        )));
        self.cached_member_hash = Some(m.clone());
        m
    }
    pub fn from_id(id: &str) -> Self {
        Self::new(hash_with_salt(id, &crate::statics::HASH_SALT))
    }
    /// The raw cookie value. Only for emitting the Set-Cookie header; never log this.
    pub fn cookie_value(&self) -> &str {
        &self.token
    }
}

#[derive(Clone)]
pub struct MemberToken {
    token: Arc<String>,
}

// Redacted for the same reason as Token: the member hash alone is enough to enumerate a
// member's posts via actual_author.
impl std::fmt::Debug for MemberToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MemberToken(redacted)")
    }
}

impl Display for MemberToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MemberToken(redacted)")
    }
}

impl MemberToken {
    pub fn new(token: Arc<String>) -> Self {
        Self { token }
    }
    pub fn post_hash(&self, id: &str) -> String {
        hash_with_salt(&self.token, id)
    }
    pub fn database_hash(&self) -> Arc<String> {
        Arc::clone(&self.token)
    }
}
