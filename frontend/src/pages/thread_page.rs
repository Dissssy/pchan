use gloo::timers::callback::Interval;
// use serde::Deserialize;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::helpers::{
    board_title::BoardTitle,
    new_post_box::PostBox,
    thread_view::{MaybeExpandableThread, ThreadView},
};

#[function_component]
pub fn ThreadPage(props: &Props) -> Html {
    // get reply value from query string
    // let reply = match use_location().map(|l| l.query::<Reply>()) {
    //     Some(Ok(query)) => query.reply,
    //     Some(Err(e)) => {
    //         gloo::console::log!(format!("{e:?}"));
    //         None
    //     }
    //     None => None,
    // };

    let rerender = use_state(|| false);

    let loadingposts = use_state(|| false);
    let handledlastpostcount = use_state(|| true);

    let thread = use_state(|| None);
    let nav = use_navigator();
    let tprops = props.clone();
    let tthread = thread.clone();
    let tloadingthreads = loadingposts.clone();
    let thandledlastthreadcount = handledlastpostcount.clone();
    let trerender = rerender.clone();
    let load_posts = Callback::from(move |_: ()| {
        thandledlastthreadcount.set(false);
        tloadingthreads.set(true);
        let ttloadingthreads = tloadingthreads.clone();
        let posts = tthread.clone();
        let props = tprops.clone();
        let tnav = nav.clone();
        let rerender = trerender.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let threads = crate::API
                .lock()
                .await
                .get_thread(&props.board_discriminator, &props.thread_id)
                .await;
            match threads {
                Ok(threads) => {
                    posts.set(Some(threads));
                }
                Err(e) => {
                    gloo::console::log!(format!("{e:?}"));
                    // redirect to 404 page
                    if let Some(n) = tnav {
                        n.replace(&crate::BaseRoute::NotFound);
                    }
                }
            }

            ttloadingthreads.set(false);
            gloo::console::log!("rerendering");
            rerender.set(!*rerender);
        });
    });
    let tloadposts = load_posts.clone();

    let backoff_max = use_state(|| 4);
    let read_backoff_max = backoff_max.clone();
    let last_post_count = use_state(|| 0);
    let backoff = use_state(|| 0);
    let read_backoff = backoff.clone();
    let bposts = thread.clone();
    let firstrun = use_state(|| true);
    if *firstrun {
        load_posts.emit(());
        firstrun.set(false);
    }
    // let ttbackoff = backoff.clone();
    let ttmax_backoff = backoff_max.clone();
    let ttloadposts = load_posts.clone();
    let manually_load_posts = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        // ttbackoff.set(4);
        ttmax_backoff.set(0);
        ttloadposts.emit(());
    });

    use_effect({
        // let bindings
        let load_posts = load_posts;
        move || {
            let interval = Interval::new(1000, move || {
                // gloo::console::log!(format!("{}/{}", *backoff, *backoff_max));
                backoff.set(*backoff + 1);
                if !*loadingposts {
                    if !*handledlastpostcount {
                        handledlastpostcount.set(true);
                        match *bposts {
                            Some(ref bposts) => {
                                if bposts.posts.len() == *last_post_count {
                                    if *backoff_max == 0 {
                                        backoff_max.set(4);
                                    } else {
                                        backoff_max.set(*backoff_max * 2);
                                    }
                                    backoff.set(0);
                                } else {
                                    backoff_max.set(4);
                                    backoff.set(0);
                                }
                                last_post_count.set(bposts.posts.len());
                            }
                            None => {
                                backoff_max.set(4);
                                backoff.set(0);
                            }
                        }
                    } else if *backoff >= (*backoff_max - 1) {
                        load_posts.emit(());
                    }
                } else {
                    gloo::console::log!("posts still loading");
                }
            });

            move || drop(interval)
        }
    });

    html! {
        <div class="thread">
            <div class="meta-shiz">
                <BoardTitle board_discriminator={props.board_discriminator.clone()}/>
                <div class="postbox">
                    <PostBox board_discriminator={props.board_discriminator.clone()} thread_id={(props.thread_id.clone(), tloadposts)} />
                </div>
            </div>
            <div class="threadposts">
                {
                    match *thread {
                        Some(ref t) => {
                            html! {
                                <>
                                    <ThreadView thread={MaybeExpandableThread::from(t.clone())} board_discriminator={props.board_discriminator.clone()} rerender={*rerender}/>
                                    <div class="reload-button">
                                        <a href="#" onclick={manually_load_posts}>
                                            {"Checking for new posts in "}{(*read_backoff_max - *read_backoff).max(0)}{" seconds"}
                                        </a>
                                    </div>
                                </>
                            }
                        }
                        None => {
                            html! {
                                <div class="loading">
                                    <p>{"Loading..."}</p>
                                </div>
                            }
                        }
                    }
                }
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub board_discriminator: String,
    pub thread_id: String,
}

// #[derive(Deserialize, Clone, PartialEq)]
// pub struct Reply {
//     reply: Option<String>,
// }
