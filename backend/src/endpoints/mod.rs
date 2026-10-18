//! Login, callback and logout.
//!
//! Replaces the hand-rolled Discord OAuth this file used to contain. That version had no
//! PKCE, no nonce, and used the `state` parameter to carry the post-login redirect target
//! rather than a CSRF token — so an attacker could force a victim into the attacker's session,
//! and `state` doubled as an open-redirect primitive. It was also keyed with a per-process
//! random value, which silently broke every in-flight login on restart.

use anyhow::{anyhow, bail, Context as _, Result};
use base64::Engine as _;
use rand::RngCore as _;
use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use warp::{Filter, Reply};

pub mod api;

/// Short-lived cookie holding the CSRF state, the OIDC nonce, the PKCE verifier and where to
/// send the user afterwards. Lives only for the duration of the round trip to Auth0.
const FLOW_COOKIE: &str = "login_flow";
const FLOW_TTL_SECONDS: i64 = 600;

#[derive(Debug, Serialize, Deserialize)]
struct LoginFlow {
    state: String,
    nonce: String,
    verifier: String,
    redirect: String,
}

pub fn other_endpoints(
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    login().or(callback()).or(logout())
}

fn random_token() -> String {
    let mut raw = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw)
}

/// S256 PKCE challenge for a verifier.
fn code_challenge(verifier: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
}

/// Only same-origin paths are accepted as a post-login destination, so `redirect` cannot be
/// turned into an open redirect to another site.
fn safe_redirect(requested: Option<String>) -> String {
    match requested {
        Some(r) if r.starts_with('/') && !r.starts_with("//") => r,
        _ => "/".to_owned(),
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginQuery {
    pub redirect: Option<String>,
}

fn login() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("login")
        .and(warp::get())
        .and(warp::query::query::<LoginQuery>())
        .map(|q: LoginQuery| {
            let cfg = crate::config::get();

            let flow = LoginFlow {
                state: random_token(),
                nonce: random_token(),
                verifier: random_token(),
                redirect: safe_redirect(q.redirect),
            };

            let authorize = format!(
                "{}?response_type=code&client_id={}&redirect_uri={}&scope=openid&state={}&nonce={}&code_challenge={}&code_challenge_method=S256",
                cfg.auth0.authorize_url(),
                urlencode(&cfg.auth0.client_id),
                urlencode(&cfg.auth0.redirect_uri(&cfg.public_url)),
                urlencode(&flow.state),
                urlencode(&flow.nonce),
                urlencode(&code_challenge(&flow.verifier)),
            );

            let cookie = match serde_json::to_string(&flow) {
                Ok(json) => format!(
                    "{FLOW_COOKIE}={}; Path=/; HttpOnly; SameSite=Lax{}; Max-Age={FLOW_TTL_SECONDS}",
                    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json),
                    if cfg.cookie_secure { "; Secure" } else { "" },
                ),
                Err(e) => {
                    log::error!("could not serialise login flow: {e}");
                    return redirect_to("/?login_error=1").into_response();
                }
            };

            warp::reply::with_header(redirect_to(&authorize), "set-cookie", cookie).into_response()
        })
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

fn callback() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("login" / "callback")
        .and(warp::get())
        .and(warp::query::query::<CallbackQuery>())
        .and(warp::cookie::optional(FLOW_COOKIE))
        .then(|q: CallbackQuery, flow: Option<String>| async move {
            match handle_callback(q, flow).await {
                Ok(reply) => reply,
                Err(e) => {
                    // Deliberately vague to the user; the detail goes to the log. Auth0
                    // subjects must never appear in a response body.
                    log::error!("login callback failed: {e:#}");
                    redirect_to("/?login_error=1").into_response()
                }
            }
        })
}

async fn handle_callback(q: CallbackQuery, flow: Option<String>) -> Result<warp::reply::Response> {
    if let Some(err) = q.error {
        bail!(
            "Auth0 returned an error: {err} ({})",
            q.error_description.unwrap_or_default()
        );
    }

    let flow = decode_flow(flow.context("no login flow cookie; the login was not started here")?)?;
    let state = q.state.context("callback had no state")?;

    // Constant-time-ish comparison is not required here (both values are ours and
    // high-entropy), but the check itself is what prevents login CSRF.
    if state != flow.state {
        bail!("state mismatch; refusing the callback");
    }

    let code = q.code.context("callback had no code")?;
    let subject = exchange_code(&code, &flow).await?;

    let mut conn = crate::POOL
        .get()
        .await
        .map_err(|e| anyhow!("could not get a database connection: {e}"))?;

    let outcome = crate::auth::resolve_subject(&mut conn, &subject).await?;

    match outcome {
        crate::auth::SubjectOutcome::Member(member_id) => {
            let token = crate::session::create(&mut conn, &member_id).await?;
            Ok(warp::reply::with_header(
                redirect_to(&flow.redirect),
                "set-cookie",
                crate::session::cookie_header(&token),
            )
            .into_response())
        }
        // No member yet. The frontend decides what to show; the backend hands over only a
        // coarse status, never anything identifying.
        crate::auth::SubjectOutcome::Pending => Ok(finish_flow("/pending")),
        crate::auth::SubjectOutcome::Blocked => Ok(finish_flow("/declined")),
        crate::auth::SubjectOutcome::NeedsRequest(ticket) => Ok(warp::reply::with_header(
            redirect_to("/request-access"),
            "set-cookie",
            crate::auth::ticket_cookie_header(&ticket),
        )
        .into_response()),
    }
}

/// Redirect and clear the login-flow cookie.
fn finish_flow(to: &str) -> warp::reply::Response {
    let cfg = crate::config::get();
    warp::reply::with_header(
        redirect_to(to),
        "set-cookie",
        format!(
            "{FLOW_COOKIE}=; Path=/; HttpOnly; SameSite=Lax{}; Max-Age=0",
            if cfg.cookie_secure { "; Secure" } else { "" }
        ),
    )
    .into_response()
}

fn decode_flow(raw: String) -> Result<LoginFlow> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(raw)
        .context("login flow cookie was not valid base64")?;
    serde_json::from_slice(&bytes).context("login flow cookie was malformed")
}

#[derive(Debug, Serialize)]
struct TokenRequest<'a> {
    grant_type: &'a str,
    client_id: &'a str,
    client_secret: &'a str,
    code: &'a str,
    redirect_uri: String,
    code_verifier: &'a str,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    id_token: String,
}

/// Exchange the authorization code and return the verified subject (`sub`) claim.
async fn exchange_code(code: &str, flow: &LoginFlow) -> Result<String> {
    let cfg = crate::config::get();
    let client = reqwest::Client::new();

    let response = client
        .post(cfg.auth0.token_url())
        .form(&TokenRequest {
            grant_type: "authorization_code",
            client_id: &cfg.auth0.client_id,
            client_secret: &cfg.auth0.client_secret,
            code,
            redirect_uri: cfg.auth0.redirect_uri(&cfg.public_url),
            code_verifier: &flow.verifier,
        })
        .send()
        .await
        .context("token exchange request failed")?;

    if !response.status().is_success() {
        let status = response.status();
        // Auth0's body says which setting is wrong (unauthorized_client, invalid_grant, a
        // redirect_uri mismatch). It describes the application's configuration, not the user.
        let body = response.text().await.unwrap_or_default();
        bail!("token endpoint returned {status}: {body}");
    }

    let tokens: TokenResponse = response
        .json()
        .await
        .context("token endpoint returned an unexpected body")?;

    crate::auth::verify_id_token(&tokens.id_token, &flow.nonce).await
}

fn logout() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("logout")
        .and(warp::post())
        .and(warp::cookie::optional(crate::session::COOKIE_NAME))
        .then(|token: Option<String>| async move {
            if let Some(token) = token {
                // Evict the cache first so the session stops working immediately rather than
                // at the end of the resolve TTL.
                crate::filters::forget_cached_session(&token).await;
                match crate::POOL.get().await {
                    Ok(mut conn) => {
                        if let Err(e) = crate::session::destroy(&mut conn, &token).await {
                            log::error!("could not destroy session: {e}");
                        }
                    }
                    Err(e) => log::error!("could not get a connection to log out: {e}"),
                }
            }

            warp::reply::with_header(
                redirect_to("/"),
                "set-cookie",
                crate::session::clear_cookie_header(),
            )
            .into_response()
        })
}

fn redirect_to(location: &str) -> warp::reply::Response {
    warp::http::Response::builder()
        .header("Location", location)
        .status(302)
        .body(String::new())
        .expect("a 302 with a Location header is always well-formed")
        .into_response()
}

fn urlencode(s: &str) -> String {
    // Percent-encode everything outside the unreserved set, so values cannot break out of the
    // query string they are placed in.
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
