use yew::prelude::*;
use yew_hooks::use_local_storage;

use crate::{helpers::REPLY_NOTIFICATIONS_KEY, ApiContext, MeContext};

/// Settings for being told about replies to the member's own posts.
#[function_component]
pub fn ReplyEditor() -> Html {
    let in_page = use_local_storage::<bool>(REPLY_NOTIFICATIONS_KEY.to_owned());
    let in_page_on = (*in_page).unwrap_or(true);
    let on_in_page = {
        let in_page = in_page.clone();
        Callback::from(move |_: Event| in_page.set(!in_page_on))
    };

    // Whether replies are also pushed to the member's devices. Kept on the server, since the
    // server is what sends the push; the page only shows and changes it.
    let me = use_context::<MeContext>().and_then(|m| m.me);
    let api = use_context::<Option<ApiContext>>()
        .flatten()
        .and_then(|c| c.api.ok());
    let push_on = use_state(|| me.as_ref().map(|m| m.reply_push).unwrap_or(false));
    let failed = use_state(|| false);
    let needs_device = use_state(|| false);
    let on_push = {
        let push_on = push_on.clone();
        let failed = failed.clone();
        let needs_device = needs_device.clone();
        Callback::from(move |_: Event| {
            let Some(api) = api.clone() else {
                return;
            };
            let wanted = !*push_on;
            let (push_on, failed, needs_device) = (push_on.clone(), failed.clone(), needs_device.clone());
            wasm_bindgen_futures::spawn_local(async move {
                match api.set_reply_push(wanted).await {
                    Ok(now) => {
                        failed.set(false);
                        push_on.set(now);
                        // The server only sends it; this device has to be willing to receive it.
                        needs_device.set(
                            now && crate::push_client::enable().await != crate::push_client::PushState::On,
                        );
                    }
                    Err(_) => failed.set(true),
                }
            });
        })
    };

    html! {
        <div class="reply-editor">
            <label>
                <input type="checkbox" checked={in_page_on} onchange={on_in_page} />
                {" Tell me in the page when someone replies to my posts"}
            </label>
            <p class="reply-note">
                {"Works in every thread, watched or not. Your browser works out which replies \
                  are to you, so the server does not need to."}
            </p>
            <label>
                <input type="checkbox" checked={*push_on} onchange={on_push} />
                {" Also push replies to my devices"}
            </label>
            <p class="reply-note">
                {"The server works out, at the moment someone replies, that you wrote the post \
                  they replied to, and does not keep or log that. A push service (Google, \
                  Mozilla or Apple) can see which device is pushed and when, so with enough \
                  effort it could tell that a device's owner wrote the post being replied to. \
                  Off unless you turn it on."}
            </p>
            if *failed {
                <p class="reply-note">{"That did not save. Try again."}</p>
            }
            if *needs_device {
                <p class="reply-note">
                    {"Replies will be pushed once this device allows notifications. Turn on \
                      \"Push notifications on this device\" above, or allow them in the browser's \
                      site settings."}
                </p>
            }
        </div>
    }
}
