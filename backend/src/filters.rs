use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};
use warp::Filter;

pub fn valid_token() -> impl Filter<Extract = (), Error = warp::Rejection> + Clone {
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
                    if crate::database::Database::is_valid_token(&mut conn, header.token)
                        .await
                        .map_err(|_| warp::reject::reject())?
                    {
                        return Ok(());
                    }
                };
                if let Some(cookie) = cookie {
                    if crate::database::Database::is_valid_token(&mut conn, cookie)
                        .await
                        .map_err(|_| warp::reject::reject())?
                    {
                        return Ok(());
                    }
                };
                Err(warp::reject::reject())
            },
        )
        .and(warp::any())
        .untuple_one()
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
