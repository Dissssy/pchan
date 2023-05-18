use yew::prelude::*;
use yew_router::prelude::*;

use crate::components::{BoardName, BoardNameType};
use crate::BaseRoute;
use crate::{api::ApiState, ApiContext};

#[function_component]
pub fn NotFound() -> Html {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            document.set_title(&format!("{}Not Found", crate::PREFIX));
        }
    }

    let api_ctx = use_context::<Option<ApiContext>>();
    let nav = use_navigator();
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
                            match api.get_boards(false).await {
                                Err(e) => {
                                    boards.set(ApiState::Error(e));
                                }
                                Ok(mut theseboards) => {
                                    theseboards.sort_by(|a, b| {
                                        a.name.to_lowercase().cmp(&b.name.to_lowercase())
                                    });
                                    boards.set(ApiState::Loaded(theseboards));
                                }
                            };
                        }
                    },
                    _ => {
                        boards.set(ApiState::ContextError(AttrValue::from("ApiContext")));
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
                                        <h2 class="error" >{"Not Found"}</h2>
                                        <p>{"The page you requested could not be found."}</p>
                                        <p>{"Maybe check out one of these awesome boards instead?"}</p>
                                    </div>
                                    <div class="home-board-list">
                                        {for boards.iter().enumerate().map(|(i, board)| html! {
                                            <BoardName board={board.clone()} view={BoardNameType::Name} first={i == 0} last={i == boards.len() - 1} prefix={"home"} />
                                        })}
                                    </div>
                                </>
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
                    }
                </div>
            </div>
        </div>
    }
}
