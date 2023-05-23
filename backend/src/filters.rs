use common::hash_with_salt;
use lazy_static::lazy_static;
use reqwest::Method;
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

lazy_static! {
    pub static ref TOKENCACHE: Arc<Mutex<HashMap<Token, Instant>>> =
        Arc::new(Mutex::new(HashMap::new()));
}

pub fn valid_token() -> impl Filter<Extract = (Token,), Error = warp::Rejection> + Clone {
    warp::any()
        .and(warp::header::optional::<Bearer>("authorization"))
        .and(warp::cookie::optional("token"))
        .and_then(
            |header: Option<Bearer>, cookie: Option<String>| async move {
                let mut conn = crate::POOL
                    .get()
                    .await
                    .map_err(|_| warp::reject::reject())?;
                if let Some(header) = header {
                    let mut t = Token::new(header.token);
                    let mut cache = TOKENCACHE.lock().await;
                    if let Entry::Occupied(e) = cache.entry(t.clone()) {
                        if e.get().elapsed().as_secs() < 60 {
                            return Ok(t);
                        } else {
                            e.remove();
                        }
                    } else if crate::database::Database::is_valid_token(&mut conn, t.member_hash())
                        .await
                        .map_err(|_| warp::reject::reject())?
                    {
                        cache.insert(t.clone(), Instant::now());
                        return Ok(t);
                    }
                };
                if let Some(cookie) = cookie {
                    let mut t = Token::new(cookie);
                    let mut cache = TOKENCACHE.lock().await;
                    if let Entry::Occupied(e) = cache.entry(t.clone()) {
                        if e.get().elapsed().as_secs() < 60 {
                            return Ok(t);
                        } else {
                            e.remove();
                        }
                    } else if crate::database::Database::is_valid_token(&mut conn, t.member_hash())
                        .await
                        .map_err(|_| warp::reject::reject())?
                    {
                        cache.insert(t.clone(), Instant::now());
                        return Ok(t);
                    }
                };
                Err(warp::reject::reject())
            },
        )
}

pub fn optional_token() -> impl Filter<Extract = (Option<Token>,), Error = warp::Rejection> + Clone
{
    warp::any()
        .and(warp::header::optional::<Bearer>("authorization"))
        .and(warp::cookie::optional("token"))
        .and_then(
            |header: Option<Bearer>, cookie: Option<String>| async move {
                let mut conn = crate::POOL
                    .get()
                    .await
                    .map_err(|_| warp::reject::reject())?;
                if let Some(header) = header {
                    let mut t = Token::new(header.token);
                    let mut cache = TOKENCACHE.lock().await;
                    if let Entry::Occupied(e) = cache.entry(t.clone()) {
                        if e.get().elapsed().as_secs() < 60 {
                            return Ok(Some(t));
                        } else {
                            e.remove();
                        }
                    } else if crate::database::Database::is_valid_token(&mut conn, t.member_hash())
                        .await
                        .map_err(|_| warp::reject::reject())?
                    {
                        cache.insert(t.clone(), Instant::now());
                        return Ok::<_, warp::reject::Rejection>(Some(t));
                    }
                };
                if let Some(cookie) = cookie {
                    let mut t = Token::new(cookie);
                    let mut cache = TOKENCACHE.lock().await;
                    if let Entry::Occupied(e) = cache.entry(t.clone()) {
                        if e.get().elapsed().as_secs() < 60 {
                            return Ok(Some(t));
                        } else {
                            e.remove();
                        }
                    } else if crate::database::Database::is_valid_token(&mut conn, t.member_hash())
                        .await
                        .map_err(|_| warp::reject::reject())?
                    {
                        cache.insert(t.clone(), Instant::now());
                        return Ok(Some(t));
                    }
                };
                Ok(None)
            },
        )
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
            if crate::statics::KNOWN_SCRAPERS.contains(&user_agent.as_str()) {
                Ok(())
            } else {
                Err(warp::reject::reject())
            }
        })
        .and(warp::any())
        .untuple_one()
}

pub fn ratelimit() -> impl Filter<Extract = (), Error = warp::Rejection> + Clone {
    warp::path::full()
        .and(valid_token())
        .and(warp::method())
        .and_then(|path: FullPath, token: Token, method| async move {
            if method == Method::GET {
                return Ok(());
            }

            // is formatted as after the url
            let rawpath = path.as_str();
            let mut path = rawpath.split('/').skip(3);
            let path = vec![path.next(), path.next(), path.next(), path.next()];

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

            let (seconds, ident): (u64, String) = match (method, path[0], path[1], path[2], path[3])
            {
                (Method::POST, Some("board"), Some(discrim), Some("thread"), Some(thread)) => (
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
                    ),
                ),
                (Method::DELETE, Some("board"), Some(_discrim), Some("post"), Some(_post)) => {
                    return Ok(())
                }
                (Method::POST, Some("board"), Some(discrim), Some("thread"), None) => (
                    10,
                    format!(
                        "make thread{}",
                        if include_board {
                            format!(" on /{}/", discrim)
                        } else {
                            String::new()
                        }
                    ),
                ),
                (Method::POST, Some("subscribe"), _, _, _) => (3, "subscribe".to_string()),
                (Method::POST, Some("file"), _, _, _) => (15, "file upload".to_string()),
                (Method::PUT, _, _, _, _) => (0, "PUT".to_string()),
                path => {
                    println!("path: {:?}", path);
                    (5, rawpath.to_string())
                }
            };

            let total_string = format!("{}|{}", ident, token);
            let mut ratelimit = crate::RATELIMIT.lock().await;
            match ratelimit.entry(total_string) {
                Entry::Occupied(mut entry) => {
                    // the entry exists, ensure the time it is set to has elapsed
                    let t = entry.get();
                    if t < &tokio::time::Instant::now() {
                        // the time has elapsed, update the time to now + seconds
                        entry.insert(
                            tokio::time::Instant::now() + tokio::time::Duration::from_secs(seconds),
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

#[derive(Debug, Clone)]
pub struct Token {
    token: Arc<String>,
    cached_member_hash: Option<MemberToken>,
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
        write!(f, "{}", self.token)
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
        let m = MemberToken::new(
            Arc::clone(&self.token),
            Arc::new(hash_with_salt(&self.token, &crate::statics::TOKEN_SALT)),
        );
        self.cached_member_hash = Some(m.clone());
        m
    }
    pub fn from_id(id: &str) -> Self {
        Self::new(hash_with_salt(id, &crate::statics::HASH_SALT))
    }
}

#[derive(Debug, Clone)]
pub struct MemberToken {
    original: Arc<String>,
    token: Arc<String>,
}

impl Display for MemberToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.token, self.original)
    }
}

impl MemberToken {
    pub fn new(original: Arc<String>, token: Arc<String>) -> Self {
        Self { original, token }
    }
    pub fn post_hash(&self, id: &str) -> String {
        hash_with_salt(&self.token, id)
    }
    pub fn member_hash(&self) -> Arc<String> {
        Arc::clone(&self.token)
    }
}
