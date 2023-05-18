use yew::prelude::*;

#[function_component]
pub fn TimezoneEditor() -> Html {
    let timezone = use_context::<UseStateHandle<chrono_tz::Tz>>();

    // let list_of_timezones: [chrono_tz::Tz; 596] = chrono_tz::TZ_VARIANTS;
    // maybe fuzzy search input box??????????

    html! {
        {
            timezone.map(|tz| tz.name()).unwrap_or("")
        }
    }
}
