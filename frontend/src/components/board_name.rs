use common::structs::SafeBoard;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::{components::MaybeLink, BaseRoute};

#[function_component]
pub fn BoardName(props: &Props) -> Html {
    let hovered = use_state(|| false);
    let location = use_route::<BaseRoute>();
    {
        let location = location.clone();
        let hovered = hovered.clone();
        use_effect_with(location, move |_| {
            hovered.set(false);
            || {}
        });
    }

    let (board_discriminator, is_thread) = location.map_or((None, false), |b| {
        (b.board_discriminator(), b.thread_id().is_some())
    });

    let (mousein, mouseout) = {
        let ahovered = hovered.clone();
        let bhovered = hovered.clone();
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
        <MaybeLink to={BaseRoute::BoardPage { board_discriminator: props.board.discriminator.clone() }}  link={(board_discriminator != Some(props.board.discriminator.clone())) || is_thread}>
            <div class={format!("{}-board-name-link{}", props.prefix, if board_discriminator == Some(props.board.discriminator.clone()) { "-selected" } else { "" })} id={id} onmouseover={mousein} onmouseout={mouseout} >
                <span class={format!("{}-board-name-container", props.prefix)} >
                    {
                        match props.view {
                            BoardNameType::Name => props.board.name.clone(),
                            BoardNameType::Descriminator => format!("/{}/", props.board.discriminator),
                            BoardNameType::Both => format!("/{}/ - {}", props.board.discriminator.clone(), props.board.name.clone()),
                        }
                    }
                </span>
                {
                    match (*hovered, &props.hover) {
                        (true, Some(state)) => {
                            html! {
                                <div class={format!("{}-board-name-hover", props.prefix)}>
                                    {
                                        match state {
                                            BoardNameType::Name => props.board.name.clone(),
                                            BoardNameType::Descriminator => format!("/{}/", props.board.discriminator),
                                            BoardNameType::Both => format!("/{}/ - {}", props.board.discriminator.clone(), props.board.name.clone()),
                                        }
                                    }
                                </div>
                            }
                        }
                        _ => html! {}
                    }
                }
            </div>
        </MaybeLink>
    }
}

#[derive(Clone, Properties, PartialEq, Debug)]
pub struct Props {
    pub board: SafeBoard,
    pub view: BoardNameType,
    #[prop_or_default]
    pub hover: Option<BoardNameType>,
    pub prefix: AttrValue,
    pub first: bool,
    pub last: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub enum BoardNameType {
    Name,
    Descriminator,
    Both,
}
