use std::rc::Rc;

use common::structs::{PushMessage, SafePost, ThreadWithPosts};
use yew::prelude::*;
// use yew_hooks::use_interval;
use yew_router::prelude::*;

use crate::{
    api::ApiState, components::*, events::use_events, helpers::SuccessfulPostContext,
    inbox::use_inbox, ApiContext, BaseRoute, Favicon,
};

/// The thread on screen. Live changes are applied through a reducer, so two arriving in the
/// same moment each build on the other instead of one overwriting it.
#[derive(Clone, PartialEq)]
struct ThreadView(ApiState<ThreadWithPosts>);

enum ThreadAction {
    Set(ApiState<ThreadWithPosts>),
    Append(SafePost),
    Remove(i64),
}

impl Reducible for ThreadView {
    type Action = ThreadAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        match (action, &self.0) {
            (ThreadAction::Set(state), _) => Rc::new(Self(state)),
            (ThreadAction::Append(post), ApiState::Loaded(thread)) => {
                // Replayed events, the poster's own post arriving twice, and a post already
                // in the fetched thread all land here.
                let known = thread.thread_post.post_number == post.post_number
                    || thread.posts.iter().any(|p| p.post_number == post.post_number);
                if known {
                    return self;
                }
                let mut thread = thread.clone();
                // Posts in different handlers can finish out of order; keep them numbered.
                let at = thread
                    .posts
                    .partition_point(|p| p.post_number < post.post_number);
                thread.posts.insert(at, post);
                thread.post_count += 1;
                Rc::new(Self(ApiState::Loaded(thread)))
            }
            (ThreadAction::Remove(number), ApiState::Loaded(thread)) => {
                if !thread.posts.iter().any(|p| p.post_number == number) {
                    return self;
                }
                let mut thread = thread.clone();
                thread.posts.retain(|p| p.post_number != number);
                thread.post_count -= 1;
                Rc::new(Self(ApiState::Loaded(thread)))
            }
            _ => self,
        }
    }
}

/// Lets the loading code below keep saying `thread.set(..)`.
#[derive(Clone)]
struct ThreadHandle(UseReducerHandle<ThreadView>);

impl ThreadHandle {
    fn set(&self, state: ApiState<ThreadWithPosts>) {
        self.0.dispatch(ThreadAction::Set(state));
    }

    fn state(&self) -> &ApiState<ThreadWithPosts> {
        &self.0 .0
    }
}

#[function_component]
pub fn ThreadPage() -> Html {
    let scroll_to_bottom = use_state(|| false);

    let route_ctx = use_route::<BaseRoute>();
    let nav = use_navigator();
    let api_ctx = use_context::<Option<ApiContext>>();
    let thread = ThreadHandle(use_reducer(|| ThreadView(ApiState::Pending)));

    // let backoff_val = use_state(|| 0.0f64);
    // let start_max = 4.0f64;
    // let backoff_max = use_state(|| start_max);
    // let mul = 1.5f64;
    let changed = use_state(|| None);

    let favicon = use_context::<Favicon>();

    let watch_hint = use_state(|| None::<bool>);

    // Reading this thread counts as having read it. This only fires while the member is
    // actually looking (see `inbox`), and the read mark only moves forward, so a thread that
    // grows while it is open keeps pace and a hidden tab does not clear anything.
    {
        let inbox = use_inbox();
        let tail = match thread.state() {
            ApiState::Loaded(t) => t
                .posts
                .iter()
                .map(|p| p.post_number)
                .chain(std::iter::once(t.thread_post.post_number))
                .max(),
            _ => None,
        };
        let here = match &route_ctx {
            Some(BaseRoute::ThreadPage {
                board_discriminator,
                thread_id,
            }) => thread_id
                .parse::<i64>()
                .ok()
                .map(|n| (board_discriminator.clone(), n)),
            _ => None,
        };
        let reading = inbox.as_ref().and_then(|i| i.active.clone());
        use_effect_with((tail, reading, here, inbox), |(tail, reading, here, inbox)| {
            if let (Some(tail), Some(reading), Some(here), Some(inbox)) =
                (tail, reading, here, inbox)
            {
                if reading == here {
                    inbox.mark_read.emit((here.0.clone(), here.1, *tail));
                }
            }
        });
    }

    {
        let api_ctx = api_ctx.clone();
        let route_ctx = route_ctx.clone();
        let thread = thread.clone();
        let changed = changed.clone();
        let nav = nav.clone();
        use_events(Callback::from(move |message: PushMessage| {
            let Some(BaseRoute::ThreadPage {
                board_discriminator,
                thread_id,
            }) = route_ctx.clone()
            else {
                return;
            };
            let here = |board: &str, number: i64| {
                board == board_discriminator && number.to_string() == thread_id
            };
            // Show the change in place and keep the reader where they were.
            let refresh = |unread: bool| {
                if let Some(window) = web_sys::window() {
                    if unread {
                        if let Some(favicon) = favicon.clone() {
                            favicon.favicon.set("/res/unread.ico".to_string());
                        }
                    }
                    changed.set(Some((window.clone(), window.scroll_x().ok(), window.scroll_y().ok())));
                }
            };
            match message {
                PushMessage::NewPost(post) if here(&post.board_discriminator, post.thread_post_number) => {
                    #[cfg(feature = "cache-post")]
                    if let Some(api) = api_ctx.clone().flatten().and_then(|a| a.api.ok()) {
                        api.insert_post_to_cache((*post).clone());
                    }
                    thread.0.dispatch(ThreadAction::Append((*post).clone()));
                    refresh(true);
                }
                PushMessage::PostDeleted(gone) if here(&gone.board_discriminator, gone.thread_post_number) => {
                    if gone.thread_deleted {
                        if let Some(nav) = nav.clone() {
                            nav.replace(&BaseRoute::BoardPage {
                                board_discriminator: gone.board_discriminator.clone(),
                            });
                        }
                    } else {
                        thread.0.dispatch(ThreadAction::Remove(gone.post_number));
                        refresh(false);
                    }
                }
                PushMessage::Resync => {
                    // The server can't say what was missed, so ask it for the thread again.
                    let (thread, changed, api_ctx) = (thread.clone(), changed.clone(), api_ctx.clone());
                    wasm_bindgen_futures::spawn_local(async move {
                        let Some(Some(api_ctx)) = api_ctx else { return };
                        let Ok(api) = api_ctx.api else { return };
                        if let Ok(fresh) = api.get_thread(&board_discriminator, &thread_id, true).await {
                            thread.set(ApiState::Loaded(fresh));
                            if let Some(window) = web_sys::window() {
                                changed.set(Some((window.clone(), window.scroll_x().ok(), window.scroll_y().ok())));
                            }
                        }
                    });
                }
                _ => {}
            }
        }));
    }

    #[cfg(feature = "cache-post")]
    {
        // Keep the cache in step with what is on screen, including live changes.
        let api_ctx = api_ctx.clone();
        use_effect_with(thread.state().clone(), move |state| {
            if let (ApiState::Loaded(t), Some(Some(ctx))) = (state, api_ctx) {
                if let Ok(api) = ctx.api {
                    api.insert_thread_to_cache(t.clone());
                }
            }
        });
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
        use_effect_with(route_ctx, |route_ctx| {
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
                                        match api.get_thread(&boardinf, &threadinf, true).await {
                                            Err(e) => {
                                                thread.set(ApiState::Error(e));
                                            }
                                            Ok(thisthread) => {
                                                if *scroll_to_bottom {
                                                    gloo_timers::callback::Timeout::new(
                                                        100,
                                                        move || {
                                                            if let Some(window) = web_sys::window()
                                                            {
                                                                window.scroll_by_with_x_and_y(
                                                                    0.0, 10000.0,
                                                                );
                                                            } else {
                                                                gloo::console::warn!(
                                                                    "Failed to scroll to bottom"
                                                                )
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
                                thread.set(ApiState::ContextError(AttrValue::from("RouteContext")));
                            }
                        }
                    },
                    _ => {
                        thread.set(ApiState::ContextError(AttrValue::from("ApiContext")));
                    }
                }
            });
        });
    }

    let on_successful_post = {
        // let manual_refresh_callback = manual_refresh_callback.clone();
        let scroll_to_bottom = scroll_to_bottom;
        let watch_hint = watch_hint.clone();
        SuccessfulPostContext {
            callback: Callback::from(move |_: SafePost| {
                // manual_refresh_callback.emit(true);
                scroll_to_bottom.set(true);
                // The post just made also watched the thread, if the member has that on.
                if crate::helpers::auto_watch_enabled() {
                    watch_hint.set(Some(true));
                }
            }),
        }
    };

    html! {
        <ContextProvider<SuccessfulPostContext> context={on_successful_post}>
            <div class={"thread-page"}>
                <Header />
                {
                    thread.state().standard_html("ThreadPage", move |thread| {
                        if let Some(window) = web_sys::window() {
                            if let Some(document) = window.document() {
                                document.set_title(&format!("{}{}", crate::PREFIX, thread.topic));
                            }
                        }
                        html! {
                            <div class={"thread-page-threads"}>
                                <Thread scroll_on_load={true} thread={thread.clone()} refresh={changed.clone()} watch_hint={watch_hint.clone()} />
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
                                <p>{format!("{}", *e)}</p>
                            </div>
                        }
                    })
                }
                <Footer />
            </div>
        </ContextProvider<SuccessfulPostContext>>
    }
}
