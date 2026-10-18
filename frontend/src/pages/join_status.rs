use yew::prelude::*;

/// Terminal states of the join flow: waiting on review, or declined.
///
/// Neither says anything a reviewer decided beyond the outcome, and the declined page does
/// not say who declined or why — that would turn a moderation decision into a message from
/// one identifiable person to another.
#[function_component]
pub fn JoinStatus(props: &Props) -> Html {
    if props.declined {
        html! {
            <div class="join-page">
                <h1>{"Not approved"}</h1>
                <p>{"Your request to join was declined. You can ask again later."}</p>
            </div>
        }
    } else {
        html! {
            <div class="join-page">
                <h1>{"Waiting on review"}</h1>
                <p>{"Your request is in the queue. Sign in again later to check."}</p>
            </div>
        }
    }
}

#[derive(Clone, Properties, PartialEq)]
pub struct Props {
    pub declined: bool,
}
