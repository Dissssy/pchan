use yew::prelude::*;

#[function_component]
pub fn ContextError(props: &Props) -> Html {
    html! {
        <div class="context-error">
            <h1>{"Context Error for "}<b>{props.cause.clone()}</b>{" in "}<b>{props.source.clone()}</b></h1>
        </div>
    }
}

#[derive(Clone, Properties, PartialEq)]
pub struct Props {
    pub cause: String,
    pub source: String,
}
