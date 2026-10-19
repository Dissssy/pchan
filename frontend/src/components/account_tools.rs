use crate::ApiContext;
use yew::prelude::*;

/// Account controls: sign out, the moderation link for those who can use it, and identity
/// rotation.
///
/// Rotation is irreversible and easy to misread as a password reset, so it is confirmed in
/// place: the tab's content is swapped for the explanation, rather than a dialog opened over
/// the settings.
#[function_component]
pub fn AccountTools() -> Html {
    let api_ctx = use_context::<Option<ApiContext>>().flatten();
    let me = use_context::<crate::MeContext>().unwrap_or_default();
    let confirming = use_state(|| false);
    let rotate_state = use_state(|| RotateState::Idle);
    let back_ref = use_node_ref();

    {
        let back_ref = back_ref.clone();
        use_effect_with(*confirming, move |confirming| {
            if *confirming {
                if let Some(el) = back_ref.cast::<web_sys::HtmlElement>() {
                    let _ = el.focus();
                }
            }
        });
    }

    let ask = {
        let confirming = confirming.clone();
        let rotate_state = rotate_state.clone();
        Callback::from(move |_: MouseEvent| {
            rotate_state.set(RotateState::Idle);
            confirming.set(true);
        })
    };
    let back = {
        let confirming = confirming.clone();
        Callback::from(move |_: MouseEvent| confirming.set(false))
    };
    let on_key = {
        let confirming = confirming.clone();
        let rotate_state = rotate_state.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Escape" && !matches!(*rotate_state, RotateState::Working) {
                e.stop_propagation();
                confirming.set(false);
            }
        })
    };

    let rotate = {
        let confirming = confirming.clone();
        let rotate_state = rotate_state.clone();
        Callback::from(move |_: MouseEvent| {
            if matches!(*rotate_state, RotateState::Working) {
                return;
            }
            let Some(api) = api_ctx.as_ref().and_then(|c| c.api.as_ref().ok()).cloned() else {
                rotate_state.set(RotateState::Failed(AttrValue::from("No API context")));
                return;
            };
            let confirming = confirming.clone();
            let rotate_state = rotate_state.clone();
            rotate_state.set(RotateState::Working);
            wasm_bindgen_futures::spawn_local(async move {
                match api.rotate_author_seed().await {
                    Ok(_) => {
                        // Those posts are no longer the member's, so forget they ever were.
                        crate::authored::clear();
                        rotate_state.set(RotateState::Done);
                        confirming.set(false);
                    }
                    // Stay on the explanation, so it can be tried again or backed out of.
                    Err(e) => rotate_state.set(RotateState::Failed(AttrValue::from(e.to_string()))),
                }
            });
        })
    };

    if *confirming {
        let working = matches!(*rotate_state, RotateState::Working);
        return html! {
            <div class="account-tools account-confirm" onkeydown={on_key}>
                <h3 class="account-confirm-title">{"Start a new posting identity?"}</h3>
                <p>
                    {"Your posts stay up, but they stop being yours: you will no longer be able \
                      to delete anything you have already posted, and they can no longer be \
                      traced to you."}
                </p>
                <p class="error">{"This cannot be undone."}</p>
                if let RotateState::Failed(e) = &*rotate_state {
                    <p class="join-error" role="alert">{ e.clone() }</p>
                }
                <div class="account-confirm-buttons">
                    <button class="account-button" ref={back_ref} onclick={back} disabled={working}>
                        {"Back"}
                    </button>
                    <button class="account-button account-button-danger" onclick={rotate} disabled={working}>
                        { if working { "Working…" } else { "Start a new identity" } }
                    </button>
                </div>
            </div>
        };
    }

    let can_moderate = me.can_moderate_anything();

    html! {
        <div class="account-tools">
            if let Some(info) = &me.me {
                <span class="account-role">{ format!("Signed in as {}", info.role.replace('_', " ")) }</span>
            }

            if can_moderate {
                <a class="account-button" href="/admin">{"Moderation"}</a>
            }

            <button class="account-button" onclick={ask}>{"Start a new posting identity"}</button>
            if matches!(*rotate_state, RotateState::Done) {
                <p class="account-note">
                    {"Done. Your earlier posts are no longer attributable to you."}
                </p>
            }

            // A form post, not fetch: logout clears the cookie and redirects, and the browser
            // should follow that navigation.
            <form method="post" action="/logout" class="account-logout">
                <button class="account-button account-button-danger" type="submit">
                    {"Sign out"}
                </button>
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
