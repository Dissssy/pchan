use common::structs::{PushMessage, SafePost, ThreadWithPosts};
use yew::prelude::*;
// use yew_hooks::use_interval;
use yew_router::prelude::*;

use crate::{
    api::ApiState, components::*, helpers::SuccessfulPostContext, hooks::use_server_sent_event,
    ApiContext, BaseRoute, Favicon,
};

#[function_component]
pub fn ThreadPage() -> Html {
    let scroll_to_bottom = use_state(|| false);

    let route_ctx = use_route::<BaseRoute>();
    let nav = use_navigator();
    let api_ctx = use_context::<Option<ApiContext>>();
    let thread: UseStateHandle<ApiState<ThreadWithPosts>> = use_state(|| ApiState::Pending);

    // let backoff_val = use_state(|| 0.0f64);
    // let start_max = 4.0f64;
    // let backoff_max = use_state(|| start_max);
    // let mul = 1.5f64;
    let changed = use_state(|| None);

    let favicon = use_context::<Favicon>();

    let mut handle = {
        let route_ctx = route_ctx.clone();
        use_server_sent_event(
            route_ctx.and_then(|r| {
                if let BaseRoute::ThreadPage {
                    board_discriminator,
                    thread_id,
                } = r
                {
                    if let Ok(thread_id) = thread_id.parse::<i64>() {
                        Some((board_discriminator, thread_id))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }),
            vec!["new_post"],
        )
    };

    {
        #[cfg(feature = "cache")]
        let api_ctx = api_ctx.clone();
        if let Some(PushMessage::NewPost(post)) = handle.get() {
            if route_ctx
                == Some(BaseRoute::ThreadPage {
                    board_discriminator: post.board_discriminator.clone(),
                    thread_id: post.thread_post_number.to_string(),
                })
            {
                let window =
                    web_sys::window().map(|w| (w.clone(), w.scroll_x().ok(), w.scroll_y().ok()));
                if let Some(favicon) = favicon {
                    favicon.favicon.set("/res/unread.ico".to_string());
                };
                changed.set(window);
                // append post to thread somehow D:
                let tthread = (*thread).clone();
                if let ApiState::Loaded(mut tthread) = tthread {
                    tthread.posts.push((*post).clone());
                    #[cfg(feature = "cache")]
                    if let Some(api) = api_ctx.flatten().and_then(|a| a.api.ok()) {
                        api.insert_post_to_cache((*post).clone());
                        api.insert_thread_to_cache(tthread.clone());
                    }

                    thread.set(ApiState::Loaded(tthread));
                }
            }
        }
    }

    // let manual_refresh_callback = {
    //     let thread = thread.clone();
    //     let route_ctx = route_ctx.clone();
    //     let backoff_val = backoff_val.clone();
    //     let backoff_max = backoff_max.clone();
    //     let api_ctx = api_ctx.clone();
    //     let favicon = favicon;
    //     let changed = changed.clone();
    //     Callback::from(move |reset: bool| {
    //         if reset {
    //             backoff_val.set(0.);
    //             backoff_max.set(start_max);
    //         }
    //         // thread.set(ApiState::Loading);
    //         let route_ctx = route_ctx.clone();
    //         let api_ctx = api_ctx.clone();
    //         let thread = thread.clone();
    //         let backoff_val = backoff_val.clone();
    //         let backoff_max = backoff_max.clone();
    //         let favicon = favicon.clone();
    //         let changed = changed.clone();
    //         wasm_bindgen_futures::spawn_local(async move {
    //             match api_ctx {
    //                 Some(Some(api_ctx)) => match api_ctx.api {
    //                     Err(e) => {
    //                         thread.set(ApiState::Error(e));
    //                     }
    //                     Ok(api) => {
    //                         if let Some((Some(boardinf), Some(threadinf))) =
    //                             route_ctx.map(|b| (b.board_discriminator(), b.thread_id()))
    //                         {
    //                             match api.get_thread(&boardinf, &threadinf, true).await {
    //                                 Err(e) => {
    //                                     thread.set(ApiState::Error(e));
    //                                 }
    //                                 Ok(thisthread) => {
    //                                     let loaded = ApiState::Loaded(thisthread);
    //                                     if loaded != *thread {
    //                                         backoff_val.set(0.);
    //                                         backoff_max.set(start_max);
    //                                         let window = web_sys::window().map(|w| {
    //                                             (w.clone(), w.scroll_x().ok(), w.scroll_y().ok())
    //                                         });
    //                                         if let Some(favicon) = favicon {
    //                                             favicon.favicon.set("/res/unread.ico".to_string());
    //                                         };
    //                                         changed.set(window);
    //                                     }
    //                                     thread.set(loaded);
    //                                 }
    //                             };
    //                         } else {
    //                             thread.set(ApiState::ContextError(AttrValue::from("BoardContext")));
    //                         }
    //                     }
    //                 },
    //                 _ => {
    //                     thread.set(ApiState::ContextError(AttrValue::from("ApiContext")));
    //                 }
    //             }
    //         });
    //     })
    // };

    // {
    //     let manual_refresh_callback = manual_refresh_callback.clone();
    //     let backoff_val = backoff_val.clone();
    //     let backoff_max = backoff_max.clone();
    //     use_interval(
    //         move || {
    //             backoff_val.set(*backoff_val + 1.);

    //             // gloo::console::log!(format!(
    //             //     "backoff_val: {}, backoff_max: {}",
    //             //     *backoff_val, *backoff_max
    //             // ));

    //             if *backoff_val >= *backoff_max {
    //                 backoff_val.set(0.);
    //                 backoff_max.set((*backoff_max) * mul);

    //                 manual_refresh_callback.emit(false);
    //             }
    //         },
    //         1000,
    //     );
    // }

    {
        let thread = thread.clone();
        let scroll_to_bottom = scroll_to_bottom.clone();
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
                                            match api.get_thread(&boardinf, &threadinf, true).await
                                            {
                                                Err(e) => {
                                                    thread.set(ApiState::Error(e));
                                                }
                                                Ok(thisthread) => {
                                                    if *scroll_to_bottom {
                                                        gloo_timers::callback::Timeout::new(
                                                            100,
                                                            move || {
                                                                if let Some(window) = web_sys::window() {
                                                                    window
                                                                    .scroll_by_with_x_and_y(
                                                                        0.0, 10000.0,
                                                                    );
                                                                } else {
                                                                    gloo::console::warn!("Failed to scroll to bottom")
                                                                }
                                                            },
                                                        )
                                                        .forget();
                                                    }
                                                    thread.set(ApiState::Loaded(thisthread));
                                                }
                                            };
                                        }
                                        (None, Some(_)) => {
                                            thread.set(ApiState::ContextError(AttrValue::from(
                                                "BoardContext",
                                            )));
                                        }
                                        (Some(_), None) => {
                                            thread.set(ApiState::ContextError(AttrValue::from(
                                                "ThreadContext",
                                            )));
                                        }
                                        (None, None) => {
                                            thread.set(ApiState::ContextError(AttrValue::from(
                                                "RouteContext",
                                            )));
                                        }
                                    }
                                } else {
                                    thread.set(ApiState::ContextError(AttrValue::from(
                                        "RouteContext",
                                    )));
                                }
                            }
                        },
                        _ => {
                            thread.set(ApiState::ContextError(AttrValue::from("ApiContext")));
                        }
                    }
                });
            },
            route_ctx,
        );
    }

    let on_successful_post = {
        // let manual_refresh_callback = manual_refresh_callback.clone();
        let scroll_to_bottom = scroll_to_bottom;
        SuccessfulPostContext {
            callback: Callback::from(move |_: SafePost| {
                // manual_refresh_callback.emit(true);
                scroll_to_bottom.set(true);
            }),
        }
    };

    html! {
        <ContextProvider<SuccessfulPostContext> context={on_successful_post}>
            <div class={"thread-page"}>
                <Header />
                {
                    thread.standard_html("ThreadPage", move |thread| {
                        if let Some(window) = web_sys::window() {
                            if let Some(document) = window.document() {
                                document.set_title(&format!("{}{}", crate::PREFIX, thread.topic));
                            }
                        }
                        html! {
                            <div class={"thread-page-threads"}>
                                <Thread thread={thread.clone()} refresh={changed.clone()} />
                                // <div class="thread-reload-button">
                                //     <span
                                //     // onclick={
                                //     //     let manual_refresh_callback = manual_refresh_callback.clone();
                                //     //     Callback::from(move |e: MouseEvent| { e.prevent_default(); manual_refresh_callback.emit(true) })
                                //     // }
                                //     >
                                //         { format!("refresh in {} seconds", ((*backoff_max).ceil() - *backoff_val).ceil() + 1.) }
                                //     </span>
                                // </div>
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
                            <div class={"thread-page-error"}>
                                <h1>{"Error"}</h1>
                                <p>{format!("{e:?}")}</p>
                            </div>
                        }
                    })
                }
                <Footer />
            </div>
        </ContextProvider<SuccessfulPostContext>>
    }
}
