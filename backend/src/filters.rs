use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};
use warp::Filter;

pub fn valid_token() -> impl Filter<Extract = (), Error = warp::Rejection> + Clone {
    warp::any()
        .and(warp::header::optional::<Bearer>("authorization"))
        .and(warp::cookie::optional("token"))
        .and_then(
            |header: Option<Bearer>, cookie: Option<String>| async move {
                println!("Validating token: {header:?} {cookie:?}");
                let mut d = crate::DATA.lock().await;
                let header_is_valid = if let Some(header) = header {
                    d.is_auth(header.token)
                        .await
                        .map_err(|_| warp::reject::reject())?
                } else {
                    false
                };
                let cookie_is_valid = if let Some(cookie) = cookie {
                    d.is_auth(cookie)
                        .await
                        .map_err(|_| warp::reject::reject())?
                } else {
                    false
                };
                if header_is_valid || cookie_is_valid {
                    Ok(())
                } else {
                    Err(warp::reject::reject())
                }
            },
        )
        .and(warp::any())
        .untuple_one()
}

pub fn priveleged_endpoint() -> impl Filter<Extract = (), Error = warp::Rejection> + Clone {
    warp::header::header::<Bearer>("authorization")
        .and_then(|header: Bearer| async move {
            if header.token != *crate::statics::TOKEN {
                println!("Attempted access with invalid token: {header}");
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
