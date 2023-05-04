use common::structs::{BoardWithThreads, SafeBoard};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::{
    api::ApiState,
    components::board_name::{BoardName, BoardNameType},
    ApiContext, BaseRoute,
};

#[function_component]
pub fn BoardTitle() -> Html {
    if *yew_hooks::use_local_storage::<bool>("verbose".to_owned()) == Some(true) {
        gloo::console::log!(format!("Refreshing BoardTitle"))
    }
    let board_ctx = use_route::<BaseRoute>();
    // let last_board_ctx = use_state(|| None);
    let api_ctx = use_context::<Option<ApiContext>>();

    let board: UseStateHandle<ApiState<BoardWithThreads>> = use_state(|| ApiState::Pending);
    {
        let board = board.clone();
        let api_ctx = api_ctx;
        // if *last_board_ctx != board_ctx {
        //     last_board_ctx.set(board_ctx.clone());
        use_effect_with_deps(
            move |board_ctx| {
                board.set(ApiState::Loading);
                let board_ctx = board_ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api_ctx {
                        Some(Some(api_ctx)) => match api_ctx.api {
                            Err(e) => {
                                board.set(ApiState::Error(e));
                            }
                            Ok(api) => {
                                if let Some(Some(boardinf)) =
                                    board_ctx.map(|b| b.board_discriminator())
                                {
                                    match api.get_board(&boardinf).await {
                                        Err(e) => {
                                            board.set(ApiState::Error(e));
                                        }
                                        Ok(thisboard) => {
                                            board.set(ApiState::Loaded(thisboard));
                                        }
                                    };
                                } else {
                                    board.set(ApiState::ContextError("BoardContext".to_string()));
                                }
                            }
                        },
                        _ => {
                            board.set(ApiState::ContextError("ApiContext".to_string()));
                        }
                    }
                });
            },
            board_ctx,
        );
        // }
    }

    board.standard_html("BoardTitle", |board| {
        html! {
            <BoardName prefix={"board-title"} first={true} last={true} board={SafeBoard::from(board.clone())} view={BoardNameType::Both} />
        }
    })
}
