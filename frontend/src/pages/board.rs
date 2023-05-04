use common::structs::BoardWithThreads;
use yew::prelude::*;
use yew_router::prelude::use_route;

use crate::{api::ApiState, components::Header, ApiContext, BaseRoute};

#[function_component]
pub fn BoardPage() -> Html {
    if *yew_hooks::use_local_storage::<bool>("verbose".to_owned()) == Some(true) {
        gloo::console::log!(format!("Refreshing BoardPage"))
    }

    let board_ctx = use_route::<BaseRoute>();
    let api_ctx = use_context::<Option<ApiContext>>();

    let board: UseStateHandle<ApiState<BoardWithThreads>> = use_state(|| ApiState::Pending);
    {
        let board = board.clone();
        let api_ctx = api_ctx;
        use_effect_with_deps(
            |board_ctx| {
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
    }
    html! {
        <div class={"board-page"}>
            <Header />
            {
                board.standard_html("BoardPage", |board| {
                    html! {
                        {
                            format!("{:?}", board.threads)
                        }
                    }
                })
            }
        </div>
    }
}
