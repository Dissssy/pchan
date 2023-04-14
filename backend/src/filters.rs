use std::{fmt::Display, str::FromStr, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use warp::Filter;

use crate::database::DBConnection;

// pub fn private_endpoint() -> impl Filter<Extract = (), Error = warp::Rejection> + Clone {
//     warp::header::header::<Bearer>("authorization")
//         .and(warp::any().map(move || crate::DATA.clone()))
//         .and_then(
//             |header: Bearer, context: Arc<Mutex<Box<dyn Strings + Send + Sync>>>| async move {
//                 if !context
//                     .lock()
//                     .await
//                     .is_whitelisted(header.token)
//                     .await
//                     .map_err(|_| warp::reject::reject())?
//                 {
//                     Err(warp::reject::reject())
//                 } else {
//                     Ok(())
//                 }
//             },
//         )
//         .and(warp::any())
//         .untuple_one()
// }

pub fn valid_token() -> impl Filter<Extract = (), Error = warp::Rejection> + Clone {
    warp::cookie::<String>("token")
        .and_then(|token: String| async move {
            if !crate::DATA
                .lock()
                .await
                .is_auth(token)
                .await
                .map_err(|_| warp::reject::reject())?
            {
                Err(warp::reject::reject())
            } else {
                Ok(())
            }
        })
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
        // formatted as  format!("Bearer {}", token) so we need to remove the `Bearer ` part
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
