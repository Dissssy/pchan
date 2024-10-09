use yew::prelude::*;

use crate::components::InviteTools;

#[function_component]
pub fn Redeem() -> Html {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            document.set_title(&format!("{}Redeem", crate::PREFIX));
        }
    }

    html! {
        <div class="valign">
            <div class="halign">
                <div class="settings">
                    <div class="redeem-code">
                        <h2>{"Redeem Code"}</h2>
                        <p>{"Click the button below to redeem your code."}</p>
                    </div>
                    <InviteTools expandable={false} />
                </div>
            </div>
        </div>
    }
}
