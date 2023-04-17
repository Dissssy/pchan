use yew::prelude::*;

#[function_component]
pub fn BoardPage(props: &Props) -> Html {
    html! {
        <h1>{props.board_discriminator.clone()}</h1>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub board_discriminator: String,
}
