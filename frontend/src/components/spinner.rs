use yew::prelude::*;
use yew_hooks::prelude::*;

#[function_component]
pub fn Spinner() -> Html {
    if *yew_hooks::use_local_storage::<bool>("verbose".to_owned()) == Some(true) {
        gloo::console::log!(format!("Refreshing Spinner"))
    }
    let show = use_state(|| false);
    {
        let show = show.clone();
        use_effect_once(move || {
            let t = gloo::timers::callback::Timeout::new(500, move || {
                show.set(true);
            });
            || {
                t.cancel();
            }
        });
    }
    html! {
        {
            if *show {
                // html! {<div class="lds-roller"><div></div><div></div><div></div><div></div><div></div><div></div><div></div><div></div></div>}
                html! { <span class="loader"></span> }
            } else {
                html! {}
            }
        }
    }
}
