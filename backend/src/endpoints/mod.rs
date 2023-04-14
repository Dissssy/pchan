use serde::{Deserialize, Serialize};
use warp::{Filter, Reply};

use common::hash_with_salt;

pub mod api;

pub fn other_endpoints(
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let login =
        warp::path!("login" / "callback")
            .and(warp::query::query::<CallbackQuery>())
            .and_then({
                move |q: CallbackQuery| {
                    async move {
                        // attempt to get the user token from the code
                        let code = q.code;
                        // make request to discord api
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
                                return Ok(warp::redirect(warp::http::Uri::from_static("/login"))
                                    .into_response());
                            }
                        }
                        .json::<DiscordTokenResponse>()
                        .await
                        {
                            Ok(r) => r.access_token,
                            Err(e) => {
                                println!("Error: {e:?}");
                                return Ok(warp::redirect(warp::http::Uri::from_static("/login"))
                                    .into_response());
                            }
                        };
                        let id = match match client
                            .get("https://discordapp.com/api/users/@me")
                            .bearer_auth(&token)
                            .send()
                            .await
                        {
                            Ok(r) => r,
                            Err(e) => {
                                println!("Error: {e}");
                                return Ok(warp::redirect(warp::http::Uri::from_static("/login"))
                                    .into_response());
                            }
                        }
                        .json::<DiscordUser>()
                        .await
                        {
                            Ok(r) => r.id,
                            Err(e) => {
                                println!("Error: {e}");
                                return Ok(warp::redirect(warp::http::Uri::from_static("/login"))
                                    .into_response());
                            }
                        };
                        let mut locked = crate::DATA.lock().await;
                        let hashed_id = hash_with_salt(&id, &crate::statics::HASH_SALT);
                        match locked.is_auth(hashed_id.clone()).await {
                            Ok(r) => {
                                if r {
                                    Ok::<_, warp::reject::Rejection>(
                                        warp::reply::with_header(
                                            warp::http::Response::builder()
                                                .header("Location", "/")
                                                .status(302)
                                                .body("".to_owned())
                                                .unwrap(),
                                            "set-cookie",
                                            format!(
                                                "token={hashed_id}; Path=/; HttpOnly; Max-Age={}",
                                                60 * 60 * 24 * 365 * 10
                                            ),
                                        )
                                        .into_response(),
                                    )
                                } else {
                                    Ok(warp::redirect(warp::http::Uri::from_static("/login"))
                                        .into_response())
                                }
                            }
                            Err(_) => Ok(warp::redirect(warp::http::Uri::from_static("/login"))
                                .into_response()),
                        }
                    }
                }
            });

    let oauth = warp::path!("login").then(|| async move {
        Ok(warp::redirect(warp::http::Uri::from_static(env!(
            "OAUTH_URL"
        ))))
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
    // pub token_type: String,
    // pub expires_in: u64,
    // pub refresh_token: String,
    // pub scope: String,
}

#[derive(Debug, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    // pub username: String,
    // pub discriminator: String,
    // pub avatar: String,
    // pub verified: bool,
    // pub email: String,
    // pub flags: u64,
    // pub banner: String,
    // pub accent_color: u64,
    // pub premium_type: u64,
    // pub public_flags: u64,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: String,
}
