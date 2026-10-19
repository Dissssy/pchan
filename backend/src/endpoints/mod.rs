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
    login()
        .or(callback())
        .or(logout())
        .or(access_request())
        .or(get_access_request())
        .or(version())
}

/// GET /api/v1/version — which frontend build is currently deployed.
///
/// Public on purpose: a logged-out visitor on the sign-in page is running the frontend too.
/// It carries no member data, only the content hash that `build.sh` puts in the wasm filename.
/// Read from the `index.html` the shell is actually serving, so it cannot disagree with what a
/// reload would load, and it needs no restart when the frontend is rebuilt.
fn version() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "v1" / "version")
        .and(warp::get())
        .then(|| async {
            let path = concat!(env!("DISTRIBUTION_PATH"), "/index.html");
            let version = match tokio::fs::read_to_string(path).await {
                Ok(html) => frontend_version(&html),
                Err(e) => {
                    log::warn!("could not read {path} for the version endpoint: {e}");
                    None
                }
            };

            let status = if version.is_some() {
                warp::http::StatusCode::OK
            } else {
                warp::http::StatusCode::SERVICE_UNAVAILABLE
            };

            // Never cached: a stale answer here defeats the point of asking.
            warp::reply::with_header(
                warp::reply::with_status(warp::reply::json(&version), status),
                "cache-control",
                "no-store",
            )
        })
}

/// The hash in `frontend_bg-<hash>.wasm`, as rewritten into `index.html` by `build.sh`.
fn frontend_version(index_html: &str) -> Option<String> {
    const PREFIX: &str = "frontend_bg-";
    let start = index_html.find(PREFIX)? + PREFIX.len();
    let hash: String = index_html[start..]
        .chars()
        .take_while(char::is_ascii_hexdigit)
        .collect();
    (!hash.is_empty()).then_some(hash)
}

#[cfg(test)]
mod version_tests {
    use super::frontend_version;

    #[test]
    fn reads_the_hash_from_the_wasm_filename() {
        let html = r#"<link rel="preload" href="/frontend_bg-65d478ab.wasm" as="fetch">"#;
        assert_eq!(frontend_version(html).as_deref(), Some("65d478ab"));
    }

    #[test]
    fn unrewritten_or_missing_is_none() {
        assert_eq!(frontend_version(r#"href="/frontend_bg.wasm""#), None);
        assert_eq!(frontend_version("<html></html>"), None);
    }
}

/// Maximum length of what an applicant writes about themselves. Long enough to explain who
/// you are, short enough not to become a storage surface.
const MAX_DISCLOSURE: usize = 2000;

/// POST /api/v1/access_request — submit a join request.
///
/// Deliberately outside `api_endpoints`: everything there sits behind a session, and an
/// applicant does not have one yet. Authorisation comes from the signed join ticket issued at
/// the end of login, which carries the request hash and nothing else.
fn access_request() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "v1" / "access_request")
        .and(warp::post())
        .and(warp::body::json::<common::structs::AccessRequest>())
        .and(warp::cookie::optional(crate::auth::ticket_cookie_name()))
        .then(
            |body: common::structs::AccessRequest, ticket: Option<String>| async move {
                let Some(ticket) = ticket else {
                    return warp::reply::with_status(
                        warp::reply::json(&"Start by signing in."),
                        warp::http::StatusCode::UNAUTHORIZED,
                    );
                };

                let request = match crate::auth::verify_ticket(&ticket) {
                    Ok(r) => r,
                    Err(e) => {
                        log::warn!("rejected a join ticket: {e}");
                        return warp::reply::with_status(
                            warp::reply::json(&"That sign-in has expired. Sign in again."),
                            warp::http::StatusCode::UNAUTHORIZED,
                        );
                    }
                };

                let disclosure = body.disclosure.trim();
                if disclosure.is_empty() {
                    return warp::reply::with_status(
                        warp::reply::json(&"Tell us something about yourself first."),
                        warp::http::StatusCode::BAD_REQUEST,
                    );
                }
                if disclosure.len() > MAX_DISCLOSURE {
                    return warp::reply::with_status(
                        warp::reply::json(&"That message is too long."),
                        warp::http::StatusCode::BAD_REQUEST,
                    );
                }

                let mut conn = match crate::POOL.get().await {
                    Ok(c) => c,
                    Err(e) => {
                        log::error!("could not get a connection for a join request: {e}");
                        return warp::reply::with_status(
                            warp::reply::json(&"Something went wrong. Try again."),
                            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                        );
                    }
                };

                match crate::auth::submit_request(&mut conn, &request, disclosure).await {
                    Ok(()) => warp::reply::with_status(
                        warp::reply::json(&"ok"),
                        warp::http::StatusCode::OK,
                    ),
                    Err(e) => {
                        log::error!("could not record a join request: {e}");
                        warp::reply::with_status(
                            warp::reply::json(&"Something went wrong. Try again."),
                            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                        )
                    }
                }
            },
        )
}

/// GET /api/v1/access_request — what this applicant has already written, if anything.
///
/// Lets the form be prefilled so reloading the page shows the pending text rather than an
/// empty box, since submitting again updates in place. Authorised by the same ticket, so an
/// applicant can only ever read back their own request.
fn get_access_request(
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "v1" / "access_request")
        .and(warp::get())
        .and(warp::cookie::optional(crate::auth::ticket_cookie_name()))
        .then(|ticket: Option<String>| async move {
            let Some(request) = ticket.and_then(|t| crate::auth::verify_ticket(&t).ok()) else {
                return warp::reply::with_status(
                    warp::reply::json(&Option::<String>::None),
                    warp::http::StatusCode::UNAUTHORIZED,
                );
            };

            let mut conn = match crate::POOL.get().await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("could not get a connection to read a join request: {e}");
                    return warp::reply::with_status(
                        warp::reply::json(&Option::<String>::None),
                        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                    );
                }
            };

            match crate::auth::get_request(&mut conn, &request).await {
                Ok(existing) => {
                    warp::reply::with_status(warp::reply::json(&existing), warp::http::StatusCode::OK)
                }
                Err(e) => {
                    log::error!("could not read a join request: {e}");
                    warp::reply::with_status(
                        warp::reply::json(&Option::<String>::None),
                        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                    )
                }
            }
        })
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
        crate::auth::SubjectOutcome::Pending(ticket) => Ok(warp::reply::with_header(
            redirect_to("/request-access"),
            "set-cookie",
            crate::auth::ticket_cookie_header(&ticket),
        )
        .into_response()),
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
