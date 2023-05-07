use common::structs::{SafePost, ThreadWithPosts};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::{
    api::ApiState,
    components::*,
    helpers::{CallbackContext, CallbackEmitterContext, SuccessfulPostContext},
    ApiContext, BaseRoute,
};

#[function_component]
pub fn ThreadPage() -> Html {
    let scroll_to_bottom = use_state(|| false);

    let route_ctx = use_route::<BaseRoute>();
    let nav = use_navigator();
    let api_ctx = use_context::<Option<ApiContext>>();

    let manual_refresh = use_state(|| false);

    let manual_refresh_callback = {
        let manual_refresh = manual_refresh.clone();
        Callback::from(move |_: ()| {
            manual_refresh.set(!*manual_refresh);
        })
    };

    let thread: UseStateHandle<ApiState<ThreadWithPosts>> = use_state(|| ApiState::Pending);
    {
        let thread = thread.clone();
        let scroll_to_bottom = scroll_to_bottom.clone();
        let manual_refresh = manual_refresh;
        let api_ctx = api_ctx;
        use_effect_with_deps(
            |(route_ctx, _)| {
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
                                                    if *scroll_to_bottom {
                                                        gloo_timers::callback::Timeout::new(
                                                            100,
                                                            move || {
                                                                web_sys::window()
                                                                    .unwrap()
                                                                    .scroll_by_with_x_and_y(
                                                                        0.0, 10000.0,
                                                                    );
                                                            },
                                                        )
                                                        .forget();
                                                    }
                                                    thread.set(ApiState::Loaded(thisthread));
                                                }
                                            };
                                        }
                                        (None, Some(_)) => {
                                            thread.set(ApiState::ContextError(
                                                "BoardContext".to_string(),
                                            ));
                                        }
                                        (Some(_), None) => {
                                            thread.set(ApiState::ContextError(
                                                "ThreadContext".to_string(),
                                            ));
                                        }
                                        (None, None) => {
                                            thread.set(ApiState::ContextError(
                                                "RouteContext".to_string(),
                                            ));
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
            (route_ctx, manual_refresh),
        );
    }

    let add_text_callback = use_state(|| None);

    let set_add_text_callback = {
        let add_text_callback = add_text_callback.clone();
        CallbackEmitterContext {
            callback: Callback::from(move |callback: Callback<common::structs::Reply>| {
                add_text_callback.set(Some(CallbackContext { callback }));
            }),
        }
    };

    let on_successful_post = {
        let manual_refresh_callback = manual_refresh_callback;
        let scroll_to_bottom = scroll_to_bottom;
        SuccessfulPostContext {
            callback: Callback::from(move |_: SafePost| {
                manual_refresh_callback.emit(());
                scroll_to_bottom.set(true);
            }),
        }
    };

    {
        // do some shenanigans to reload posts every 5 (with exponential backoff) seconds later or on click, resetting the timer and backoff
    }

    html! {
        <ContextProvider<SuccessfulPostContext> context={on_successful_post}>
            <ContextProvider<CallbackEmitterContext> context={set_add_text_callback}>
                <ContextProvider<Option<CallbackContext>> context={(*add_text_callback).clone()}>
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
                </ContextProvider<Option<CallbackContext>>>
            </ContextProvider<CallbackEmitterContext>>
        </ContextProvider<SuccessfulPostContext>>
    }
}
