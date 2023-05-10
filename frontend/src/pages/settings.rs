use yew::prelude::*;
use yew_hooks::use_local_storage;

use crate::components::ThemeEditor;

#[function_component]
pub fn Settings() -> Html {
    let token = use_local_storage::<String>("token".to_string());

    let on_click = {
        let token = token.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            if let Some(the) = token.as_ref() {
                #[cfg(web_sys_unstable_apis)]
                web_sys::Clipboard::write_with_string(the).unwrap();
            }
        })
    };

    html! {
        <div class="valign">
            <div class="halign">
                <div class="settings">
                    <ThemeEditor />
                    {
                        if token.is_some() {
                            html! {
                                <span onclick={on_click} class="token-button">{"Click here to copy your token (DO NOT SHARE WITH ANYONE)"}</span>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>
            </div>
        </div>
    }
}
