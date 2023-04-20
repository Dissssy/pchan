use gloo::timers::callback::Interval;
use serde::Deserialize;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::helpers::{board_title::BoardTitle, new_post_box::PostBox, post_container::PostView};

#[function_component]
pub fn BoardPage(props: &Props) -> Html {
    let threads = use_state(Vec::new);
    let loadingthreads = use_state(|| false);
    let handledlastthreadcount = use_state(|| true);

    let nav = use_navigator();
    let tthreads = threads.clone();
    let tloadingthreads = loadingthreads.clone();
    let thandledlastthreadcount = handledlastthreadcount.clone();
    let tprops = props.clone();
    let load_threads = Callback::from(move |_: ()| {
        thandledlastthreadcount.set(false);
        tloadingthreads.set(true);
        let ttthreads = tthreads.clone();
        let ttprops = tprops.clone();
        let ttloadingthreads = tloadingthreads.clone();
        let ttnav = nav.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let fetch = gloo_net::http::Request::get(&format!(
                "/api/v1/board/{}",
                ttprops.board_discriminator
            ))
            .send()
            .await;
            match fetch {
                Ok(f) => match f.json::<BoardWithThreads>().await {
                    Ok(boardses) => {
                        ttthreads.set(boardses.threads);
                    }
                    Err(e) => {
                        gloo::console::log!(format!("{e:?}"));
                        // redirect to 404 page
                        if let Some(n) = ttnav {
                            n.replace(&crate::BaseRoute::NotFound);
                        }
                    }
                },
                Err(e) => {
                    gloo::console::log!(format!("{e:?}"));
                }
            };
            ttloadingthreads.set(false);
        });
    });

    // manually trigger the load threads callback on mount, then an exponential backoff if no new threads are found

    let backoff_max = use_state(|| 5);
    let last_thread_count = use_state(|| 0);
    let backoff = use_state(|| 0);
    let bthreads = threads.clone();
    let firstrun = use_state(|| true);
    if *firstrun {
        load_threads.emit(());
        firstrun.set(false);
    }
    use_effect({
        // let bindings
        let load_threads = load_threads;
        move || {
            let interval = Interval::new(1000, move || {
                // gloo::console::log!(format!("{}/{}", *backoff, *backoff_max));
                backoff.set(*backoff + 1);
                if !*loadingthreads {
                    if !*handledlastthreadcount {
                        handledlastthreadcount.set(true);
                        if bthreads.len() == *last_thread_count {
                            backoff_max.set(*backoff_max * 2);
                            backoff.set(0);
                        } else {
                            backoff_max.set(5);
                            backoff.set(0);
                        }
                        last_thread_count.set(bthreads.len());
                    } else if *backoff >= *backoff_max {
                        load_threads.emit(());
                    }
                } else {
                    gloo::console::log!("threads still loading");
                }
            });

            move || drop(interval)
        }
    });

    html! {
        <div class="board">
            <div class="board-title">
                <div class="board-title">
                    <BoardTitle board_discriminator={props.board_discriminator.clone()}/>
                </div>
            </div>
            <div class="postbox">
                <PostBox board_discriminator={props.board_discriminator.clone()} />
            </div>
            <div class="board-threads">
                {
                    for threads.iter().map(|t| {
                        html! {
                            <div class="board-thread">
                                <div class="board-thread-post">
                                    <PostView post={t.thread_post.clone()} hyperlink={Some(())} />
                                </div>
                                <div class="board-thread-reply">
                                    {
                                        for t.posts.iter().map(|p| {
                                            html! {
                                                <div class="board-thread-reply-post">
                                                    <PostView post={p.clone()} />
                                                </div>
                                            }
                                        })
                                    }
                                </div>
                            </div>
                        }
                    })
                }
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub board_discriminator: String,
}

#[derive(Deserialize, Clone)]
pub struct BoardWithThreads {
    pub threads: Vec<ThreadWithPosts>,
    pub name: String,
}

#[derive(Deserialize, Clone)]
pub struct ThreadWithPosts {
    pub thread_post: SafePost,
    pub posts: Vec<SafePost>,
}

#[derive(Deserialize, Clone, PartialEq)]
pub struct SafePost {
    pub post_number: i64,
    pub image: Option<String>,
    pub author: Option<String>,
    pub content: String,
    pub timestamp: String,
    pub replies: Vec<i64>,
}
