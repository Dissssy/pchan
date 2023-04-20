use yew::prelude::*;

#[function_component]
pub fn StartsWithClass(props: &Props) -> Html {
    match props.map.iter().find(|(s, _)| props.text.starts_with(s)) {
        Some((_, class)) => html! {
            <span class={class}>{props.text.clone()}</span>
        },
        None => {
            html! {
                <>{props.text.clone()}</>
            }
        }
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    pub text: String,
    pub map: Vec<(String, String)>,
}
