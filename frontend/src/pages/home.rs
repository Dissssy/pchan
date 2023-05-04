use yew::prelude::*;

use crate::components::{BoardName, BoardNameType};
use crate::{api::ApiState, ApiContext};

#[function_component]
pub fn Home() -> Html {
    if *yew_hooks::use_local_storage::<bool>("verbose".to_owned()) == Some(true) {
        gloo::console::log!(format!("Refreshing Home"))
    }
    let api_ctx = use_context::<Option<ApiContext>>();
    let boards: UseStateHandle<ApiState<Vec<common::structs::SafeBoard>>> =
        use_state(|| ApiState::Pending);
    {
        let boards = boards.clone();
        let api_ctx = api_ctx;
        if *boards == ApiState::Pending {
            boards.set(ApiState::Loading);
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
        }
    }
    html! {
        <div class="valign">
            <div class="halign">
                <div class="home">
                    {
                        boards.standard_html("Home", |boards| {
                            html! {
                                <>
                                    <div class="home-title">
                                        <h1>{"PChan"}</h1>
                                    </div>
                                    <div class="home-board-list">
                                        {for boards.iter().enumerate().map(|(i, board)| html! {
                                            <BoardName board={board.clone()} view={BoardNameType::Name} first={i == 0} last={i == boards.len() - 1} prefix={"home"} />
                                        })}
                                    </div>
                                </>
                            }
                        })
                    }
                </div>
            </div>
        </div>
    }
}
