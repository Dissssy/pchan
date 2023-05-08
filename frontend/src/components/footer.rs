use yew::prelude::*;

use crate::components::BannerAd;

#[function_component]
pub fn Footer() -> Html {
    // will contain more later, for now just a simple banner ad

    html! {
        <div class="footer">
            <BannerAd />
        </div>
    }
}
