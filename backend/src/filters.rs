use std::{collections::hash_map::Entry, fmt::Display, str::FromStr};

use reqwest::Method;
use serde::{Deserialize, Serialize};
use warp::{path::FullPath, Filter};

pub fn valid_token() -> impl Filter<Extract = (String,), Error = warp::Rejection> + Clone {
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
                    if crate::database::Database::is_valid_token(&mut conn, header.token.clone())
                        .await
                        .map_err(|_| warp::reject::reject())?
                    {
                        return Ok(header.token);
                    }
                };
                if let Some(cookie) = cookie {
                    if crate::database::Database::is_valid_token(&mut conn, cookie.clone())
                        .await
                        .map_err(|_| warp::reject::reject())?
                    {
                        return Ok(cookie);
                    }
                };
                Err(warp::reject::reject())
            },
        )
}

pub fn optional_token() -> impl Filter<Extract = (Option<String>,), Error = warp::Rejection> + Clone
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
                    if crate::database::Database::is_valid_token(&mut conn, header.token.clone())
                        .await
                        .map_err(|_| warp::reject::reject())?
                    {
                        return Ok::<_, warp::reject::Rejection>(Some(header.token));
                    }
                };
                if let Some(cookie) = cookie {
                    if crate::database::Database::is_valid_token(&mut conn, cookie.clone())
                        .await
                        .map_err(|_| warp::reject::reject())?
                    {
                        return Ok(Some(cookie));
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
        .and_then(|path: FullPath, token: String, method| async move {
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
                            "".to_string()
                        },
                        if include_thread {
                            format!(" in {}", thread)
                        } else {
                            "".to_string()
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
                            "".to_string()
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
