use crate::ApiContext;
use yew::prelude::*;

/// Where someone lands after signing in with Auth0 without being a member yet, and where a
/// pending applicant lands again on subsequent sign-ins.
///
/// The text box is the only channel through which an applicant identifies themselves — the
/// backend never asks Auth0 for an email and never stores the subject, so whatever is typed
/// here is all a reviewer will ever see.
///
/// Submitting updates in place rather than queueing a second request, so when there is already
/// text on file the form is prefilled with it and presents itself as an edit.
#[function_component]
pub fn RequestAccess() -> Html {
    let api_ctx = use_context::<Option<ApiContext>>().flatten();
    let text = use_state(String::new);
    let status = use_state(|| Status::Loading);
    // Whether a request is already on file, which decides the wording throughout.
    let existing = use_state(|| false);

    {
        let api_ctx = api_ctx.clone();
        let text = text.clone();
        let status = status.clone();
        let existing = existing.clone();
        use_effect_with((), move |_| {
            let Some(api) = api_ctx.as_ref().and_then(|c| c.api.as_ref().ok()).cloned() else {
                status.set(Status::Editing);
                return;
            };
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(Some(prior)) = api.get_access_request().await {
                    text.set(prior);
                    existing.set(true);
                }
                status.set(Status::Editing);
            });
        });
    }

    let on_input = {
        let text = text.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                text.set(el.value());
            }
        })
    };

    let on_submit = {
        let api_ctx = api_ctx.clone();
        let text = text.clone();
        let status = status.clone();
        Callback::from(move |_: MouseEvent| {
            let Some(api) = api_ctx.as_ref().and_then(|c| c.api.as_ref().ok()).cloned() else {
                status.set(Status::Failed(AttrValue::Static("No connection to the server.")));
                return;
            };
            let body = text.trim().to_owned();
            if body.is_empty() {
                status.set(Status::Failed(AttrValue::Static(
                    "Say a little about yourself first.",
                )));
                return;
            }

            status.set(Status::Sending);
            let status = status.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api.submit_access_request(body).await {
                    Ok(_) => status.set(Status::Sent),
                    Err(e) => status.set(Status::Failed(AttrValue::from(e.to_string()))),
                }
            });
        })
    };

    let had_existing = *existing;

    match &*status {
        Status::Loading => html! {
            <div class="join-page"><crate::components::Spinner /></div>
        },
        Status::Sent => html! {
            <div class="join-page">
                <h1>{ if had_existing { "Request updated" } else { "Request sent" } }</h1>
                <p>{"A moderator will take a look. Sign in again later to check, or to change \
                     what you wrote."}</p>
            </div>
        },
        st => {
            let sending = matches!(st, Status::Sending);
            html! {
                <div class="join-page">
                    <h1>{ if had_existing { "Your request" } else { "Request access" } }</h1>
                    if had_existing {
                        <p>
                            {"Your request is in the queue. You can still change what it says — \
                              editing replaces the original rather than adding a second request."}
                        </p>
                    } else {
                        <p>
                            {"This board is invite-only. Tell the moderators who you are and why \
                              you want in — it is the only thing they will see about you."}
                        </p>
                    }
                    <textarea
                        class="join-disclosure"
                        rows="6"
                        placeholder="Who are you?"
                        value={(*text).clone()}
                        oninput={on_input}
                        disabled={sending}
                    />
                    if let Status::Failed(msg) = st {
                        <p class="join-error">{msg.clone()}</p>
                    }
                    <button class="join-button" onclick={on_submit} disabled={sending}>
                        {
                            match (sending, had_existing) {
                                (true, _) => "Sending...",
                                (false, true) => "Update request",
                                (false, false) => "Send request",
                            }
                        }
                    </button>
                </div>
            }
        }
    }
}

enum Status {
    Loading,
    Editing,
    Sending,
    Sent,
    Failed(AttrValue),
}
