use yew::prelude::*;
use yew_hooks::prelude::*;
use yew_router::prelude::use_navigator;

use crate::{
    api::ApiState,
    components::board_name::{BoardName, BoardNameType},
    ApiContext, BaseRoute,
};

#[function_component]
pub fn BoardSelectBar() -> Html {
    let api_ctx = use_context::<Option<ApiContext>>();
    let nav = use_navigator();
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
                            match api.get_boards(false).await {
                                Err(e) => {
                                    boards.set(ApiState::Error(e));
                                }
                                Ok(mut theseboards) => {
                                    theseboards.sort_by(|a, b| {
                                        a.discriminator
                                            .to_lowercase()
                                            .cmp(&b.discriminator.to_lowercase())
                                    });
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
    }).unwrap_or_else(|e| {
        match nav {
            Some(nav) => {
                nav.replace(&BaseRoute::NotFound);
            }
            None => {
                gloo::console::error!("Failed to navigate to /404");
            }
        }
        html! {
            <div class={"board-page-error"}>
                <h1>{"Error"}</h1>
                <p>{format!("{e:?}")}</p>
            </div>
        }
    })
    //     }
    // }
}
