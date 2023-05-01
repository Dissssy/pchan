use common::structs::SafeBoard;
use yew::prelude::*;

#[function_component]
pub fn BoardName(props: &Props) -> Html {
    let hovered = use_state(|| false);

    let (mousein, mouseout) = {
        let ahovered = hovered.clone();
        let bhovered = hovered;
        (
            Callback::from(move |_e: MouseEvent| ahovered.set(true)),
            Callback::from(move |_e: MouseEvent| bhovered.set(false)),
        )
    };

    let id = format!(
        "{}-{}",
        props.prefix,
        match (props.first, props.last) {
            (true, true) => "board-name-only",
            (true, false) => "board-name-first",
            (false, true) => "board-name-last",
            (false, false) => "board-name-middle",
        }
    );

    html! {
        <a draggable="false" href={format!("/{}/", props.board.discriminator.clone())} class={format!("{}-board-name-link", props.prefix)} id={id}>
            <span class={format!("{}-board-name-container", props.prefix)} onmouseover={mousein} onmouseout={mouseout}>
                {
                    match props.view {
                        BoardNameType::Name => props.board.name.clone(),
                        BoardNameType::Descriminator => props.board.discriminator.clone(),
                        BoardNameType::Both => format!("/{}/ - {}", props.board.discriminator.clone(), props.board.name.clone()),
                    }
                }
            </span>
        </a>
    }
}

#[derive(Clone, Properties, PartialEq)]
pub struct Props {
    pub board: SafeBoard,
    pub view: BoardNameType,
    pub prefix: String,
    pub first: bool,
    pub last: bool,
}

#[derive(Clone, PartialEq)]
pub enum BoardNameType {
    Name,
    Descriminator,
    Both,
}
