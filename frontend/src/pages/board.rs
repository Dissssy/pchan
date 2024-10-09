use common::structs::{BoardWithThreads, SafePost};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::{api::ApiState, components::*, helpers::SuccessfulPostContext, ApiContext, BaseRoute};

#[function_component]
pub fn BoardPage() -> Html {
    let board_ctx = use_route::<BaseRoute>();
    let nav = use_navigator();
    let api_ctx = use_context::<Option<ApiContext>>();

    let board: UseStateHandle<ApiState<BoardWithThreads>> = use_state(|| ApiState::Pending);
    {
        let board = board.clone();
        let api_ctx = api_ctx;
        use_effect_with(board_ctx, |board_ctx| {
            board.set(ApiState::Loading);
            let board_ctx = board_ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api_ctx {
                    Some(Some(api_ctx)) => match api_ctx.api {
                        Err(e) => {
                            board.set(ApiState::Error(e));
                        }
                        Ok(api) => {
                            if let Some(Some(boardinf)) = board_ctx.map(|b| b.board_discriminator())
                            {
                                match api.get_board(&boardinf, false).await {
                                    Err(e) => {
                                        board.set(ApiState::Error(e));
                                    }
                                    Ok(thisboard) => {
                                        board.set(ApiState::Loaded(thisboard));
                                    }
                                };
                            } else {
                                board.set(ApiState::ContextError(AttrValue::from("BoardContext")));
                            }
                        }
                    },
                    _ => {
                        board.set(ApiState::ContextError(AttrValue::from("ApiContext")));
                    }
                }
            });
        });
    }

    // let callback = if let Some(c) = use_context::<Option<CallbackContext>>().flatten() {
    //     let nav = nav.clone();
    //     let modified = c.callback.reform(move |r: common::structs::Reply| {
    //         // we need to navigate to the new thread D:
    //         let nav = nav.clone();
    //         if let Some(nav) = nav {
    //             nav.replace(&BaseRoute::ThreadPage {
    //                 board_discriminator: r.board_discriminator.clone(),
    //                 thread_id: r.thread_post_number.clone().unwrap(),
    //             });
    //         }
    //         r
    //     });
    //     Some(CallbackContext { callback: modified })
    // } else {
    //     None
    // };

    let successful_post = {
        let nav = nav.clone();

        SuccessfulPostContext {
            callback: Callback::from(move |p: SafePost| {
                if let Some(nav) = &nav {
                    nav.push(&BaseRoute::ThreadPage {
                        board_discriminator: p.board_discriminator.clone(),
                        thread_id: p.thread_post_number.to_string(),
                    });
                }
            }),
        }
    };

    html! {
        // <ContextProvider<Option<CallbackContext>> context={callback}>
        <ContextProvider<SuccessfulPostContext> context={successful_post}>
            // <ContextProvider<CallbackEmitterContext> context={set_add_text_callback}>
            //     <ContextProvider<Option<CallbackContext>> context={(*add_text_callback).clone()}>
                    <div class={"board-page"}>
                        <Header />
                        {
                            board.standard_html("BoardPage", |board| {
                                if let Some(window) = web_sys::window() {
                                    if let Some(document) = window.document() {
                                        document.set_title(&format!("{}/{}/ - {}", crate::PREFIX, board.info.discriminator, board.info.name));
                                    }
                                }
                                html! {
                                    <div class={"board-page-threads"}>
                                        {
                                            board.threads.iter().map(|thread| {
                                                html! {
                                                    <Thread scroll_on_load={false} thread={thread.clone()} />
                                                }
                                            }).collect::<Html>()
                                        }
                                    </div>
                                }
                            }).unwrap_or_else(|e| {
                                if let Some(window) = web_sys::window() {
                                    if let Some(document) = window.document() {
                                        document.set_title(&format!("{}Error", crate::PREFIX));
                                    }
                                }
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
                                        <p>{format!("{}", *e)}</p>
                                    </div>
                                }
                            })
                        }
                        <Footer />
                    </div>
            //     </ContextProvider<Option<CallbackContext>>>
            // </ContextProvider<CallbackEmitterContext>>
        </ContextProvider<SuccessfulPostContext>>
        // </ContextProvider<Option<CallbackContext>>>
    }
}
