use std::str::FromStr;

use yew::prelude::*;

use crate::{
    components::ContextError,
    helpers::{on_change_select_element, on_input_to_string},
};

#[function_component]
pub fn TimezoneEditor() -> Html {
    let timezone = use_context::<UseStateHandle<chrono_tz::Tz>>();
    let search = use_state(String::new);
    let list = use_state(|| {
        chrono_tz::TZ_VARIANTS
            .iter()
            .map(|tz| tz.name())
            .collect::<Vec<&'static str>>()
    });
    match timezone {
        None => {
            html! {
                <ContextError source={"TimezoneEditor"} cause={"No timezone context found"} />
            }
        }
        Some(tz) => {
            let name = tz.name();
            let list_of_timezones = list
                .iter()
                .filter(|thistz| {
                    if search.len() > 0 {
                        thistz.to_lowercase().contains(&search.to_lowercase())
                    } else {
                        true
                    }
                })
                .copied()
                .collect::<Vec<&'static str>>();
            // maybe fuzzy search input box??????????

            html! {
                <div class="timezone-editor">
                    <span>{format!("Current Timezone: {}", name)}</span>
                    <input
                        type="text"
                        placeholder="Search"
                        value={(*search).clone()}
                        oninput={move |e: InputEvent| {
                            if let Some(e) = on_input_to_string(e) {
                                search.set(e.value());
                            }
                        }}
                    />
                    <select
                        onchange={move |e: Event| {
                            if let Some(change) = on_change_select_element(e) {
                                if let Ok(timezone) = chrono_tz::Tz::from_str(change.value().as_str()) {
                                    tz.set(timezone);
                                } else {
                                    gloo::console::error!("Failed to parse timezone");
                                }
                            } else {
                                gloo::console::error!("Failed to get select element");
                            }
                        }}
                    >
                        {for list_of_timezones.iter().map(|thistz| {
                            html! {
                                <option value={*thistz} selected={name == *thistz}>{thistz}</option>
                            }
                        })}
                    </select>
                </div>
            }
        }
    }
}
