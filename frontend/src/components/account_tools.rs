use crate::ApiContext;
use yew::prelude::*;

/// Account controls: sign out, the moderation link for those who can use it, and identity
/// rotation.
#[function_component]
pub fn AccountTools() -> Html {
    let api_ctx = use_context::<Option<ApiContext>>().flatten();
    let me = use_context::<crate::MeContext>().unwrap_or_default();
    let rotate_state = use_state(|| RotateState::Idle);

    let rotate = {
        let api_ctx = api_ctx.clone();
        let rotate_state = rotate_state.clone();
        Callback::from(move |_: MouseEvent| {
            // Irreversible and easy to misread as a password reset, so make the cost explicit
            // before doing it.
            let confirmed = web_sys::window()
                .and_then(|w| {
                    w.confirm_with_message(
                        "Start a new posting identity?\n\nYour posts stay up, but they stop \
                         being yours: you will no longer be able to delete anything you have \
                         already posted. This cannot be undone.",
                    )
                    .ok()
                })
                .unwrap_or(false);
            if !confirmed {
                return;
            }

            let Some(api) = api_ctx.as_ref().and_then(|c| c.api.as_ref().ok()).cloned() else {
                return;
            };
            let rotate_state = rotate_state.clone();
            rotate_state.set(RotateState::Working);
            wasm_bindgen_futures::spawn_local(async move {
                match api.rotate_author_seed().await {
                    Ok(_) => rotate_state.set(RotateState::Done),
                    Err(e) => rotate_state.set(RotateState::Failed(AttrValue::from(e.to_string()))),
                }
            });
        })
    };

    let can_moderate = me.can_moderate_site();

    html! {
        <div class="account-tools">
            <h2>{"Account"}</h2>
            if let Some(info) = &me.me {
                <p class="account-role">{format!("Signed in as: {}", info.role)}</p>
            }

            if can_moderate {
                <a class="join-button" href="/moderation">{"Review join requests"}</a>
            }

            <button class="join-button" onclick={rotate}>{"Start a new posting identity"}</button>
            {
                match &*rotate_state {
                    RotateState::Idle => html!{},
                    RotateState::Working => html!{ <p class="account-note">{"Working..."}</p> },
                    RotateState::Done => html!{
                        <p class="account-note">
                            {"Done. Your earlier posts are no longer attributable to you."}
                        </p>
                    },
                    RotateState::Failed(e) => html!{ <p class="join-error">{e.clone()}</p> },
                }
            }

            // A form post, not fetch: logout clears the cookie and redirects, and the browser
            // should follow that navigation.
            <form method="post" action="/logout" class="account-logout">
                <button class="join-button join-button-danger" type="submit">{"Sign out"}</button>
            </form>
        </div>
    }
}

enum RotateState {
    Idle,
    Working,
    Done,
    Failed(AttrValue),
}
