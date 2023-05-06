use crate::{pages::Settings, theme_data::ThemeData};
use yew::prelude::*;

#[function_component]
pub fn SettingsButton() -> Html {
    if *yew_hooks::use_local_storage::<bool>("verbose".to_owned()) == Some(true) {
        gloo::console::log!(format!("Refreshing SettingsButton"))
    }
    let current_theme = use_context::<UseStateHandle<Option<ThemeData>>>();

    let popup = use_state(|| false);

    let on_click = {
        let popup = popup.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            popup.set(!*popup);
        })
    };

    html! {
        <div class="toggle-theme">
            {
                if *popup {
                    // show the <Settings /> component as a popup with a close button
                    html! {
                        <>
                            <div class="popup-blur" id="ignore-transition" onclick={on_click.clone()}/>
                            <div class="popup" id="ignore-transition">
                                <div class="popup-content">
                                    <a href="/" class="popup-close-button" onclick={on_click.clone()}>{"❌"}</a>
                                    <Settings />
                                </div>
                            </div>
                        </>
                    }
                } else {
                    // show the <Settings /> component as a link
                    html! {
                        <a href="/settings" onclick={on_click} >{"⚙️"}</a>
                    }
                }
            }
            <style>
                {format!("
                    :root {{
                        {}
                    }}
                    ", current_theme.as_ref().and_then(|theme| theme.as_ref().map(|theme| theme.to_css_str())).unwrap_or_default())
                }
            </style>
        </div>
    }
}
