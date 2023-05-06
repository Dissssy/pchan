use common::structs::ThreadWithPosts;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::{api::ApiState, components::*, ApiContext, BaseRoute};

#[function_component]
pub fn ThreadPage() -> Html {
    if *yew_hooks::use_local_storage::<bool>("verbose".to_owned()) == Some(true) {
        gloo::console::log!(format!("Refreshing ThreadPage"))
    }

    let route_ctx = use_route::<BaseRoute>();
    let nav = use_navigator();
    let api_ctx = use_context::<Option<ApiContext>>();

    let thread: UseStateHandle<ApiState<ThreadWithPosts>> = use_state(|| ApiState::Pending);
    {
        let thread = thread.clone();
        let api_ctx = api_ctx;
        use_effect_with_deps(
            |route_ctx| {
                thread.set(ApiState::Loading);
                let route_ctx = route_ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api_ctx {
                        Some(Some(api_ctx)) => match api_ctx.api {
                            Err(e) => {
                                thread.set(ApiState::Error(e));
                            }
                            Ok(api) => {
                                if let Some(route_ctx) = route_ctx {
                                    match (route_ctx.board_discriminator(), route_ctx.thread_id()) {
                                        (Some(boardinf), Some(threadinf)) => {
                                            match api.get_thread(&boardinf, &threadinf).await {
                                                Err(e) => {
                                                    thread.set(ApiState::Error(e));
                                                }
                                                Ok(thisthread) => {
                                                    thread.set(ApiState::Loaded(thisthread));
                                                }
                                            };
                                        }
                                        (None, Some(_)) => {
                                            thread.set(ApiState::ContextError("BoardContext".to_string()));
                                        }
                                        (Some(_), None) => {
                                            thread.set(ApiState::ContextError("ThreadContext".to_string()));
                                        }
                                        (None, None) => {
                                            thread.set(ApiState::ContextError("RouteContext".to_string()));
                                        }
                                    }
                                } else {
                                    thread.set(ApiState::ContextError("RouteContext".to_string()));
                                }
                            }
                        },
                        _ => {
                            thread.set(ApiState::ContextError("ApiContext".to_string()));
                        }
                    }
                });
            },
            route_ctx,
        );
    }
    
    html! {
        <div class={"thread-page"}>
            <Header />
            {
                thread.standard_html("ThreadPage", |thread| {
                    html! {
                        <div class={"thread-page-threads"}>
                            <Thread thread={thread.clone()} />
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
                        <div class={"thread-page-error"}>
                            <h1>{"Error"}</h1>
                            <p>{format!("{e:?}")}</p>
                        </div>
                    }
                })
            }
        </div>
    }
}