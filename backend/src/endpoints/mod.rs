use serde::{Deserialize, Serialize};
use warp::{Filter, Reply};

use crate::{decode_checksum_str, filters::Token};

pub mod api;

pub fn other_endpoints(
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let login = warp::path!("login" / "callback")
        .and(warp::query::query::<CallbackQuery>())
        .and_then({
            move |q: CallbackQuery| async move {
                let code = q.code;

                let client = reqwest::Client::new();
                let token = match match client
                    .post("https://discord.com/api/v10/oauth2/token")
                    .form(&DiscordTokenForm {
                        client_id: env!("CLIENT_ID").to_owned(),
                        client_secret: env!("CLIENT_SECRET").to_owned(),
                        grant_type: "authorization_code".to_owned(),
                        redirect_uri: "https://pchan.p51.nl/login/callback".to_owned(),
                        code,
                    })
                    .send()
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        println!("Error: {e}");
                        return Ok(warp::reply::json(&e.to_string()).into_response());
                    }
                }
                .text()
                .await
                {
                    Ok(r) => match serde_json::from_str::<DiscordTokenResponse>(&r) {
                        Ok(r) => r.access_token,
                        Err(e) => {
                            println!("Error: {e:?}");
                            return Ok(warp::reply::json(&format!(
                                "Error: {e:?} while parsing {r}"
                            ))
                            .into_response());
                        }
                    },
                    Err(e) => {
                        println!("Error: {e:?}");
                        return Ok(warp::reply::json(&e.to_string()).into_response());
                    }
                };
                let mut token = match match client
                    .get("https://discordapp.com/api/users/@me")
                    .bearer_auth(&token)
                    .send()
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        println!("Error: {e}");
                        return Ok(warp::reply::json(&e.to_string()).into_response());
                    }
                }
                .text()
                .await
                {
                    Ok(r) => match serde_json::from_str::<DiscordUser>(&r) {
                        Ok(r) => Token::from_id(&r.id),
                        Err(e) => {
                            println!("Error: {e:?}");
                            return Ok(warp::reply::json(&format!(
                                "Error: {e:?} while parsing {r}"
                            ))
                            .into_response());
                        }
                    },
                    Err(e) => {
                        println!("Error: {e}");
                        return Ok(warp::reply::json(&e.to_string()).into_response());
                    }
                };
                let is_auth = {
                    let mut conn = crate::POOL
                        .get()
                        .await
                        .map_err(|_| warp::reject::reject())?;

                    crate::database::Database::is_valid_token(&mut conn, token.member_hash()).await
                };

                match is_auth {
                    Ok(true) => Ok::<_, warp::reject::Rejection>(
                        warp::reply::with_header(
                            warp::http::Response::builder()
                                .header(
                                    "Location",
                                    decode_checksum_str(&q.state).unwrap_or("/".to_string()),
                                )
                                .status(302)
                                .body(String::new())
                                .expect("Failed to build login redirect response"),
                            "set-cookie",
                            format!(
                                "token={token}; Path=/; HttpOnly; Max-Age={}",
                                60 * 60 * 24 * 365 * 10
                            ),
                        )
                        .into_response(),
                    ),
                    _ => Ok(warp::http::Response::builder()
                        .header("Location", "/unauthorized")
                        .status(302)
                        .body(String::new())
                        .expect("Failed to build login redirect response")
                        .into_response()),
                }
            }
        });

    let oauth = warp::path!("login")
        .and(warp::query::query::<LoginQuery>())
        .then(|LoginQuery { redirect }| async move {
            warp::http::Response::builder()
                .header(
                    "Location",
                    format!("{}&state={}", env!("OAUTH_URL"), redirect),
                )
                .status(302)
                .body(String::new())
                .expect("Failed to build login redirect response")
                .into_response()
        });
    oauth.or(login)
}

#[derive(Debug, Serialize)]
pub struct DiscordTokenForm {
    pub client_id: String,
    pub client_secret: String,
    pub grant_type: String,
    pub code: String,
    pub redirect_uri: String,
}

#[derive(Debug, Deserialize)]
pub struct DiscordTokenResponse {
    pub access_token: String,
}

#[derive(Debug, Deserialize)]
pub struct DiscordUser {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginQuery {
    pub redirect: String,
}
