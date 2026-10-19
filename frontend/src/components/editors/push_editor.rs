use yew::prelude::*;

use crate::push_client::{self, PushState};

/// Turn push notifications on or off for this device.
#[function_component]
pub fn PushEditor() -> Html {
    let state = use_state(|| None::<PushState>);

    {
        let state = state.clone();
        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                state.set(Some(push_client::state().await));
            });
        });
    }

    let onchange = {
        let state = state.clone();
        Callback::from(move |_: Event| {
            let state = state.clone();
            let turning_on = *state != Some(PushState::On);
            wasm_bindgen_futures::spawn_local(async move {
                state.set(Some(if turning_on {
                    push_client::enable().await
                } else {
                    push_client::disable().await
                }));
            });
        })
    };

    let (checked, usable, note) = match *state {
        None => (false, false, "Checking…"),
        Some(PushState::On) => (true, true, "This device will be sent notifications."),
        Some(PushState::Off) => (false, true, "Notifications are only shown while the site is open."),
        Some(PushState::Denied) => (
            false,
            false,
            "Your browser is blocking notifications for this site. Allow them in the browser's site settings, then come back.",
        ),
        Some(PushState::Unsupported) => (false, false, "This browser cannot show push notifications."),
        Some(PushState::Error) => (false, true, "That did not work. Try again."),
    };

    html! {
        <div class="push-editor">
            <label>
                <input type="checkbox" checked={checked} disabled={!usable} {onchange} />
                {" Push notifications on this device"}
            </label>
            <p class="push-note">{ note }</p>
        </div>
    }
}
