use yew::prelude::*;
use yew_hooks::use_local_storage;

use crate::{components::Toggle, helpers::REPLY_NOTIFICATIONS_KEY, ApiContext, MeContext};

/// Settings for being told about replies to the member's own posts.
#[function_component]
pub fn ReplyEditor() -> Html {
    let in_page = use_local_storage::<bool>(REPLY_NOTIFICATIONS_KEY.to_owned());
    let in_page_on = (*in_page).unwrap_or(true);
    let on_in_page = {
        let in_page = in_page.clone();
        Callback::from(move |wanted: bool| in_page.set(wanted))
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
    let saving = use_state(|| false);
    let on_push = {
        let push_on = push_on.clone();
        let failed = failed.clone();
        let needs_device = needs_device.clone();
        let saving = saving.clone();
        Callback::from(move |wanted: bool| {
            let Some(api) = api.clone() else {
                return;
            };
            let (push_on, failed, needs_device) = (push_on.clone(), failed.clone(), needs_device.clone());
            let saving = saving.clone();
            saving.set(true);
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
                saving.set(false);
            });
        })
    };

    html! {
        <>
            <Toggle checked={in_page_on} onchange={on_in_page} label="Tell me in the page when someone replies to my posts">
                {"Works in every thread, watched or not. Your browser works out which replies \
                  are to you, so the server does not need to."}
            </Toggle>
            <Toggle checked={*push_on} onchange={on_push} pending={*saving} warn={true} label="Also push replies to my devices">
                <p>
                    {"The server works out, at the moment someone replies, that you wrote the post \
                      they replied to, and does not keep or log that. A push service (Google, \
                      Mozilla or Apple) can see which device is pushed and when, so with enough \
                      effort it could tell that a device's owner wrote the post being replied to. \
                      Off unless you turn it on."}
                </p>
                if *failed {
                    <p>{"That did not save. Try again."}</p>
                }
                if *needs_device {
                    <p>
                        {"Replies will be pushed once this device allows notifications. Turn on \
                          \"Push notifications on this device\" above, or allow them in the \
                          browser's site settings."}
                    </p>
                }
            </Toggle>
        </>
    }
}
