use yew::prelude::*;

use super::Reply;

#[function_component]
pub fn LazyPost(props: &Props) -> Html {
    // TODO: make clickable, when clicked EMBED the post into the page awesome style

    html! {}
}

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    pub reply: Reply,
}
