use serde::{Deserialize, Serialize};
use warp::{Filter, Reply};
use anyhow::Result;

use crate::{decode_checksum_str, filters::{MemberToken, Token}};

pub mod api;

pub fn other_endpoints(
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let login = warp::path!("login" / "callback")
        .and(warp::query::query::<CallbackQuery>())
        .and_then({
            move |q: CallbackQuery| async move {
                let code = q.code;

                let client = reqwest::Client::new();
                let token = match fetch_discord_token(&client, &code).await {
                    Ok(token) => token,
                    Err(e) => {
                        datalust_logger::rich_anyhow_logging::error(&e);
                        return Ok(warp::reply::json(&e.to_string()).into_response());
                    }
                };
                let user_id = match fetch_discord_user_id(&client, &token).await {
                    Ok(user_id) => user_id,
                    Err(e) => {
                        datalust_logger::rich_anyhow_logging::error(&e);
                        return Ok(warp::reply::json(&e.to_string()).into_response());
                    }
                };
                let mut token = Token::from_id(&user_id);

                match validate_token(token.member_hash()).await {
                    Ok(true) => {
                        return Ok::<_, warp::reject::Rejection>(
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
                        );
                    }
                    Ok(false) => {
                        datalust_logger::rich_anyhow_logging::trace(&anyhow::anyhow!(
                            "Token is not valid"
                        ));
                    }
                    Err(e) => {
                        datalust_logger::rich_anyhow_logging::error(&e);
                    }
                };
                return Ok(unauthorized_redirect());
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

async fn fetch_discord_token(
    client: &reqwest::Client,
    code: &str,
) -> Result<String> {
    let response = client
        .post("https://discord.com/api/v10/oauth2/token")
        .form(&DiscordTokenForm {
            client_id: env!("CLIENT_ID").to_owned(),
            client_secret: env!("CLIENT_SECRET").to_owned(),
            grant_type: "authorization_code".to_owned(),
            redirect_uri: "https://pchan.p51.nl/login/callback".to_owned(),
            code: code.to_owned(),
        })
        .send()
        .await?;

    let token_response: DiscordTokenResponse = response.json().await?;
    Ok(token_response.access_token)
}

async fn fetch_discord_user_id(
    client: &reqwest::Client,
    token: &str,
) -> Result<String> {
    let response = client
        .get("https://discordapp.com/api/users/@me")
        .bearer_auth(token)
        .send()
        .await?;

    let user: DiscordUser = response.json().await?;
    Ok(user.id)
}

async fn validate_token(
    token: MemberToken,
) -> Result<bool> {
    let mut conn = crate::POOL
        .get()
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to get connection from pool: {e}"
            )
        })?;

    crate::database_bindings::Database::is_valid_token(
        &mut conn,
        token,
    )
    .await
}

fn unauthorized_redirect() -> warp::reply::Response {
    warp::http::Response::builder()
        .header("Location", "/unauthorized")
        .status(302)
        .body(String::new())
        .expect("Failed to build login redirect response")
        .into_response()
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
