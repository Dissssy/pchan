use base64::Engine as _;
use lazy_static::lazy_static;
use serde::Deserialize;
use std::{
    collections::{hash_map::Entry, HashMap},
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

/// A viewer with site-admin authority: boards, moderator grants, the audit log.
pub fn site_admin() -> impl Filter<Extract = (database::Viewer,), Error = warp::Rejection> + Clone {
    viewer().and_then(|v: database::Viewer| async move {
        if v.role.is_admin() {
            Ok(v)
        } else {
            Err(warp::reject::reject())
        }
    })
}

/// A viewer who may review the join queue and act on reported posts.
///
/// The role lives in Postgres, never in Auth0: a mod with the Auth0 dashboard could correlate
/// login timestamps against post timestamps and deanonymise a small population.
pub fn site_mod() -> impl Filter<Extract = (database::Viewer,), Error = warp::Rejection> + Clone {
    viewer().and_then(|v: database::Viewer| async move {
        if v.role.can_moderate_site() {
            Ok(v)
        } else {
            Err(warp::reject::reject())
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
                // Only on a cache miss, so this is at most one write per cache TTL per
                // session rather than one per request.
                crate::session::touch(&mut conn, &token).await;
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
    // Their live streams were authorised by sessions that no longer count. Closing them makes
    // the browser reconnect, and the reconnect is refused.
    crate::push::lock(&crate::PUSH_NOTIFS).disconnect_member(member);
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

// priveleged_endpoint is gone with the shared static TOKEN it compared, in non-constant
// time, against an Authorization header. Admin actions are gated by the admin role.

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
                let path = [
                    path.next(),
                    path.next(),
                    path.next(),
                    path.next(),
                    path.next(),
                ];

                // POST /board/?/thread/?
                // DELETE /board/?/post/?
                // POST /board/?/thread
                // POST /subscribe
                // POST /unsubscribe
                // POST /file
                let include_thread = std::env::var("THREAD_SPECIFIC_RATELIMIT")
                    .map(|v| v.parse::<bool>().unwrap_or_default())
                    .unwrap_or_default();
                let include_board = include_thread
                    || std::env::var("BOARD_SPECIFIC_RATELIMIT")
                        .map(|v| v.parse::<bool>().unwrap_or_default())
                        .unwrap_or_default();

                let (seconds, ident): (u64, Arc<str>) =
                    match (method, path[0], path[1], path[2], path[3], path[4]) {
                        (
                            Method::POST,
                            Some("board"),
                            Some(discrim),
                            Some("thread"),
                            Some(thread),
                            _,
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
                            _,
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
                        (Method::POST, Some("board"), Some(discrim), Some("thread"), None, _) => (
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
                        (Method::POST, Some("subscribe"), _, _, _, _) => (3, "subscribe".into()),
                        // About 0.1ms of database and 3-27us per post asked about (see the timing test
                        // in database::keys), at 200 posts a request: once a second is generous.
                        (Method::POST, Some("authored"), _, _, _, _) => (1, "authored".into()),
                        (Method::POST, Some("client_errors"), _, _, _, _) => {
                            (2, "client errors".into())
                        }
                        (Method::POST, Some("unsubscribe"), _, _, _, _) => {
                            (3, "unsubscribe".into())
                        }
                        (Method::POST, Some("file"), _, _, _, _) => (15, "file upload".into()),
                        // Reporting is cheap to do and easy to weaponise, so it gets the longest
                        // cooldown of anything here.
                        (Method::POST, Some("board"), _, Some("post"), _, Some("report")) => {
                            (120, "report".into())
                        }
                        (Method::PUT, _, _, _, _, _) => (0, "PUT".into()),
                        (Method::PATCH, Some("consume_code"), None, None, None, _) => {
                            (60, "consume code".into())
                        }
                        path => {
                            // Logged at trace, not info: this prints board and post for every
                            // unmatched mutating request, which is noise at best.
                            log::trace!("unbucketed path: {:?}", path);
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


// Token and MemberToken are gone: credentials are no longer values the app derives and
// passes around. A session cookie resolves to a database::Viewer (see crate::session).

