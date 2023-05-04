use yew::prelude::*;

use crate::components::ThemeEditor;

#[function_component]
pub fn Settings() -> Html {
    if *yew_hooks::use_local_storage::<bool>("verbose".to_owned()) == Some(true) {
        gloo::console::log!(format!("Refreshing Settings"))
    }
    html! {
        <div class="valign">
            <div class="halign">
                <div class="settings">
                    <ThemeEditor />
                </div>
            </div>
        </div>
    }
}
