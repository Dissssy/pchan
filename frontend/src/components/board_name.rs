use common::structs::SafeBoard;
// use gloo::console::log;
use yew::prelude::*;
// use yew_router::prelude::use_navigator;
use yew_router::prelude::*;

use crate::BaseRoute;

#[function_component]
pub fn BoardName(props: &Props) -> Html {
    if *yew_hooks::use_local_storage::<bool>("verbose".to_owned()) == Some(true) {
        gloo::console::log!(format!("Refreshing BoardName: Props = {:?}", props))
    }
    let hovered = use_state(|| false);

    let board_context = use_route::<BaseRoute>();

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

    // let on_click = {
    //     let nav = use_navigator();
    //     let board = props.board.clone();
    //     Callback::from(move |e: MouseEvent| {
    //         e.prevent_default();
    //         if let Some(nav) = &nav {
    //             nav.push(&BaseRoute::BoardPage {
    //                 board_discriminator: board.discriminator.clone(),
    //             });
    //         } else {
    //             log!("Error: No navigator, redirecting manually");
    //             match web_sys::window().map(|w| {
    //                 w.location()
    //                     .set_href(&format!("/{}/", board.discriminator.clone()))
    //             }) {
    //                 Some(Ok(_)) => {}
    //                 Some(Err(e)) => log!(format!("Error: {:?}", e)),
    //                 None => log!("Error: No window"),
    //             }
    //         }
    //     })
    // };

    html! {
        //<a onclick={on_click} draggable="false" href={format!("/{}/", props.board.discriminator.clone())} class={format!("{}-board-name-link{}", props.prefix, if board_context.map(|b| b.discriminator ) == Some(props.board.discriminator.clone()) { "-selected" } else { "" })} id={id}>
        <Link<BaseRoute> to={BaseRoute::BoardPage { board_discriminator: props.board.discriminator.clone() }} >
            <div class={format!("{}-board-name-link{}", props.prefix, if board_context.and_then(|b| b.board_discriminator() ) == Some(props.board.discriminator.clone()) { "-selected" } else { "" })} id={id}>
                <span class={format!("{}-board-name-container", props.prefix)} onmouseover={mousein} onmouseout={mouseout} >
                    {
                        match props.view {
                            BoardNameType::Name => props.board.name.clone(),
                            BoardNameType::Descriminator => format!("/{}/", props.board.discriminator),
                            BoardNameType::Both => format!("/{}/ - {}", props.board.discriminator.clone(), props.board.name.clone()),
                        }
                    }
                </span>
                {
                    if *hovered {
                        if let Some(state) = &props.hover {
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
                        } else {
                            html! {}
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
        </Link<BaseRoute>>
    }
}

#[derive(Clone, Properties, PartialEq, Debug)]
pub struct Props {
    pub board: SafeBoard,
    pub view: BoardNameType,
    pub hover: Option<BoardNameType>,
    pub prefix: String,
    pub first: bool,
    pub last: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub enum BoardNameType {
    Name,
    Descriminator,
    Both,
}
