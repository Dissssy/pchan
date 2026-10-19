use yew::prelude::*;

/// Shown wherever the API reports the viewer is not signed in.
///
/// A plain link rather than a fetch: `/login` is a server-side redirect into Auth0, so the
/// browser has to navigate rather than XHR it. The current path is passed along so the user
/// comes back to where they were.
#[function_component]
pub fn SignIn() -> Html {
    let href = sign_in_href();

    html! {
        <div class="sign-in">
            <h1>{"PChan"}</h1>
            <p>{"This board is private. Sign in to continue."}</p>
            <a class="sign-in-button" href={href}>{"Sign in"}</a>
        </div>
    }
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
