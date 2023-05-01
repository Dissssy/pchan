use yew::prelude::*;

use crate::{components::context_error::ContextError, ApiContext};

use crate::components::board_name::{BoardName, BoardNameType};

#[function_component]
pub fn Home() -> Html {
    let api = use_context::<Option<ApiContext>>().unwrap();
    let boards = use_state(|| None);
    {
        let boards = boards.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(v) = api {
                v.api.unwrap().dispatch_get_boards(boards)
            }
        });
    }

    html! {
        <div class="valign">
            <div class="halign">
                <div class="home">
                    {
                        match &*boards {
                            None => html! {
                                <div class="loading">
                                    <h1>{"Loading..."}</h1>
                                </div>
                            },
                            Some(Err(None)) => html! {
                                <ContextError cause="Api" source="Home"/>
                            },
                            Some(Err(Some(e))) => html! {
                                <div class="error">
                                    <h1>{"Error"}</h1>
                                    <p>{format!("{e:?}")}</p>
                                </div>
                            },
                            Some(Ok(boards)) => {
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
                            }
                        }
                    }
                </div>
            </div>
        </div>
    }
}
