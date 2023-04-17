use common::structs::User;
use stylist::css;
use yew::prelude::*;
use yew_router::prelude::*;

#[function_component]
pub fn Sidebar() -> Html {
    // this is the sidebar component
    // it will show a list of routes that the user can quickly navigate to from the Home route enum as well as a dropdown for subroutes
    // it will also show the user's avatar and username at the top of the sidebar, stored in the user's local storage.

    // first a logo that is a link to the home page
    html! {
        <a href="/" class={css!("display: inline-block; width: 0%; height: 0%;")}>
            <img src="/res/icon.png" alt="logo" class={css!("width: 50px; margin: 0.3em; padding: 0px; border-radius: 50%; box-shadow: 0 0 0.5em 0.1em ${bg}; background-color: ${bg}; color: ${tx};", bg=crate::PALETTE.primary, tx=crate::PALETTE.text)} />
        </a>
    }
}
