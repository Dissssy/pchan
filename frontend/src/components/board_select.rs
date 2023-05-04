use yew::prelude::*;
use yew_hooks::prelude::*;

use crate::{
    api::ApiState,
    components::board_name::{BoardName, BoardNameType},
    ApiContext,
};

#[function_component]
pub fn BoardSelectBar() -> Html {
    if *yew_hooks::use_local_storage::<bool>("verbose".to_owned()) == Some(true) {
        gloo::console::log!(format!("Refreshing BoardSelectBar"))
    }

    let api_ctx = use_context::<Option<ApiContext>>();
    let boards: UseStateHandle<ApiState<Vec<common::structs::SafeBoard>>> =
        use_state(|| ApiState::Loading);
    {
        let boards = boards.clone();
        let api_ctx = api_ctx;
        use_effect_once(move || {
            wasm_bindgen_futures::spawn_local(async move {
                match api_ctx {
                    Some(Some(api_ctx)) => match api_ctx.api {
                        Err(e) => {
                            boards.set(ApiState::Error(e));
                        }
                        Ok(api) => {
                            match api.get_boards().await {
                                Err(e) => {
                                    boards.set(ApiState::Error(e));
                                }
                                Ok(theseboards) => {
                                    boards.set(ApiState::Loaded(theseboards));
                                }
                            };
                        }
                    },
                    _ => {
                        boards.set(ApiState::ContextError("ApiContext".to_string()));
                    }
                }
            });
            || {}
        });
    }

    // match use_route::<BaseRoute>() {
    //     None => {
    //         html! {
    //             <ContextError cause={"BoardContext"} source={"BoardSelectBar"} />
    //         }
    //     }
    //     Some(_) => {
    boards.standard_html("BoardSelectBar", |boards| {
        html! {
            <div class="board-select-bar">
                {for boards.iter().enumerate().map(|(i, board)| html! {
                    <BoardName board={board.clone()} view={BoardNameType::Descriminator} hover={BoardNameType::Name} first={i == 0} last={i == boards.len() - 1} prefix={"board-select"} />
                })}
            </div>
        }
    })
    //     }
    // }
}
