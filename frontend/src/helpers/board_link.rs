use yew::prelude::*;

#[function_component]
pub fn BoardLink(props: &Props) -> Html {
    let expanded = use_state(|| false);

    let texpanded = expanded.clone();
    let on_mouseover = Callback::from(move |_| {
        texpanded.set(true);
    });

    let texpanded = expanded.clone();
    let on_mouseout = Callback::from(move |_| {
        texpanded.set(false);
    });

    html! {
        <a class={ if props.board.discriminator == props.board_discriminator { "current-navbar-link" } else { "navbar-link" }} href={format!("/{}/", props.board.discriminator.clone())} onmouseover={on_mouseover} onmouseout={on_mouseout}>
            <span title={props.board.name.clone()}>
                    {
                        if *expanded {
                            props.board.name.clone()
                        } else {
                            format!("/{}/", props.board.discriminator.clone())
                        }
                    }
            </span>
        </a>
    }
}

#[derive(Clone, Properties, PartialEq)]
pub struct Props {
    pub board: common::structs::SafeBoard,
    pub board_discriminator: String,
}
