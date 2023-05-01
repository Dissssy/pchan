use yew::prelude::*;

use crate::components::theme_editor::ThemeEditor;

#[function_component]
pub fn Settings() -> Html {
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
