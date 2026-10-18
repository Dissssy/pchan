use yew::prelude::*;

/// Shown wherever the API reports the viewer is not signed in.
///
/// A plain link rather than a fetch: `/login` is a server-side redirect into Auth0, so the
/// browser has to navigate rather than XHR it. The current path is passed along so the user
/// comes back to where they were.
#[function_component]
pub fn SignIn() -> Html {
    let href = {
        let path = web_sys::window()
            .and_then(|w| w.location().pathname().ok())
            .unwrap_or_else(|| "/".to_owned());
        format!("/login?redirect={}", urlencode(&path))
    };

    html! {
        <div class="sign-in">
            <h1>{"PChan"}</h1>
            <p>{"This board is private. Sign in to continue."}</p>
            <a class="sign-in-button" href={href}>{"Sign in"}</a>
        </div>
    }
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
