use yew::prelude::*;
use yew_hooks::use_local_storage;

use crate::{components::Toggle, helpers::AUTO_WATCH_KEY};

#[function_component]
pub fn AutoWatchEditor() -> Html {
    let stored = use_local_storage::<bool>(AUTO_WATCH_KEY.to_owned());
    let enabled = (*stored).unwrap_or(false);
    let onchange = {
        let stored = stored.clone();
        Callback::from(move |wanted: bool| stored.set(wanted))
    };

    html! {
        <Toggle checked={enabled} {onchange} label="Watch threads I post in">
            {"Off by default. Replies to your posts notify you either way, in every thread. \
              Watched threads notify you of every new post, and are stored against your \
              account, so with this on the list holds the threads you have posted in."}
        </Toggle>
    }
}
