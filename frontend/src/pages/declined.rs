use yew::prelude::*;

/// Shown when a join request was declined.
///
/// Says nothing about who decided or why: a moderation outcome should not become a message
/// from one identifiable person to another.
#[function_component]
pub fn Declined() -> Html {
    html! {
        <div class="join-page">
            <h1>{"Not approved"}</h1>
            <p>{"Your request to join was declined. You can ask again later."}</p>
        </div>
    }
}
