use yew::prelude::*;

use crate::{
    components::Toggle,
    push_client::{self, PushState},
};

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

    let busy = use_state(|| false);

    let onchange = {
        let state = state.clone();
        let busy = busy.clone();
        Callback::from(move |turning_on: bool| {
            let state = state.clone();
            let busy = busy.clone();
            busy.set(true);
            wasm_bindgen_futures::spawn_local(async move {
                state.set(Some(if turning_on {
                    push_client::enable().await
                } else {
                    push_client::disable().await
                }));
                busy.set(false);
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
        <Toggle {checked} {onchange} disabled={!usable} pending={*busy} label="Push notifications on this device">
            { note }
        </Toggle>
    }
}
