use yew::prelude::*;

/// How long after one automatic redirect another is refused. If a sign-in "succeeds" but the
/// viewer still comes back unauthenticated (cookie blocked, clock skew), redirecting again
/// would loop forever between here and Auth0.
const AUTO_REDIRECT_COOLDOWN_MS: f64 = 30_000.0;
const AUTO_REDIRECT_KEY: &str = "pchan_auto_signin";

/// Shown wherever the API reports the viewer is not signed in.
///
/// Sends the browser straight to `/login`, which is a server-side redirect into Auth0, so it
/// has to navigate rather than XHR. The current path is passed along so the user comes back
/// to where they were. The button stays as the fallback for every case where redirecting
/// automatically would be wrong: a failed login (`?login_error`), a deliberate logout
/// (`?logged_out`), or a redirect that already happened moments ago.
#[function_component]
pub fn SignIn() -> Html {
    let href = sign_in_href();
    let notice = notice_from_query();
    let redirecting = notice.is_none() && auto_redirect_allowed();

    {
        let href = href.clone();
        use_effect_with(redirecting, move |redirecting| {
            if *redirecting {
                remember_auto_redirect();
                if let Some(w) = web_sys::window() {
                    let _ = w.location().set_href(&href);
                }
            }
        });
    }

    html! {
        <div class="sign-in">
            <h1>{"PChan"}</h1>
            if redirecting {
                <p>{"Taking you to sign in\u{2026}"}</p>
            } else {
                if let Some(n) = notice {
                    <p>{n}</p>
                }
                <p>{"This board is private. Sign in to continue."}</p>
            }
            <a class="sign-in-button" href={href}>{"Sign in"}</a>
        </div>
    }
}

fn notice_from_query() -> Option<&'static str> {
    let search = web_sys::window()?.location().search().ok()?;
    if search.contains("login_error") {
        Some("Sign-in did not complete. Please try again.")
    } else if search.contains("logged_out") {
        Some("You have been signed out.")
    } else {
        None
    }
}

fn now_ms() -> f64 {
    js_sys::Date::now()
}

fn auto_redirect_allowed() -> bool {
    use gloo::storage::{SessionStorage, Storage};
    match SessionStorage::get::<f64>(AUTO_REDIRECT_KEY) {
        Ok(last) => now_ms() - last > AUTO_REDIRECT_COOLDOWN_MS,
        // Nothing stored, or storage unavailable. Unavailable storage cannot record the
        // cooldown either, which is the loop case, so only an absent key counts as allowed.
        Err(gloo::storage::errors::StorageError::KeyNotFound(_)) => true,
        Err(_) => false,
    }
}

fn remember_auto_redirect() {
    use gloo::storage::{SessionStorage, Storage};
    let _ = SessionStorage::set(AUTO_REDIRECT_KEY, now_ms());
}

/// The login URL that returns to the current page.
///
/// The query string is part of "where they were": an invite link is `/redeem?invite_code=...`,
/// and an approved applicant's member is only created by the very login this leads to, so
/// dropping the query would lose the code at exactly the moment it first becomes usable.
pub fn sign_in_href() -> String {
    let location = web_sys::window().map(|w| w.location());
    let path = location
        .as_ref()
        .and_then(|l| l.pathname().ok())
        .unwrap_or_else(|| "/".to_owned());
    let search = location
        .and_then(|l| l.search().ok())
        .unwrap_or_default();
    format!("/login?redirect={}", urlencode(&format!("{path}{search}")))
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
