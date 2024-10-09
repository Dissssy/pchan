use crate::{pages::Settings, BaseRoute, ThemeData};
use yew::prelude::*;
use yew_router::hooks::use_route;

#[function_component]
pub fn SettingsButton() -> Html {
    let popup = use_state(|| false);

    let location = use_route::<BaseRoute>();

    let on_click = {
        let popup = popup.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            popup.set(!*popup);
        })
    };

    let theme = use_context::<ThemeData>();

    match location {
        Some(BaseRoute::Settings) => {
            html! {
                <>
                    <div class="toggle-theme">
                        <a href="/" >{"󰅙"}</a>
                    </div>
                    <style>
                        {format!("
                            :root {{
                                {}
                            }}
                            ", theme.as_ref().map(|theme| theme.css()).unwrap_or_default())
                        }
                    </style>
                </>
            }
        }
        _ => {
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
                                            <a href="/" class="popup-close-button" onclick={on_click.clone()}>{"󱧼"}</a>
                                            <Settings />
                                        </div>
                                    </div>
                                </>
                            }
                        } else {
                            // show the <Settings /> component as a link
                            html! {
                                <a href="/settings" onclick={on_click} >{"󱧽"}</a>
                            }
                        }
                    }
                    <style>
                        {format!("
                            :root {{
                                {}
                            }}
                            ", theme.as_ref().map(|theme| theme.css()).unwrap_or_default())
                        }
                    </style>
                </div>
            }
        }
    }
}
