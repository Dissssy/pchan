use yew::prelude::*;
use yew_router::prelude::use_navigator;

use crate::helpers::{
    banner::Banner,
    board_title::BoardTitle,
    boards_navbar::NavBar,
    new_post_box::PostBox,
    thread_view::{MaybeExpandableThread, ThreadView},
};

#[function_component]
pub fn BoardPage(props: &Props) -> Html {
    let threads = use_state(Vec::new);
    let loadingthreads = use_state(|| false);
    let handledlastthreadcount = use_state(|| true);

    let nav = use_navigator();
    let tthreads = threads.clone();
    let tloadingthreads = loadingthreads;
    let thandledlastthreadcount = handledlastthreadcount;
    let tprops = props.clone();
    let load_threads = Callback::from(move |_: ()| {
        thandledlastthreadcount.set(false);
        tloadingthreads.set(true);
        let ttthreads = tthreads.clone();
        let ttprops = tprops.clone();
        let ttloadingthreads = tloadingthreads.clone();
        let ttnav = nav.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let b = crate::API
                .lock()
                .await
                .get_board(&ttprops.board_discriminator)
                .await;
            match b {
                Ok(b) => {
                    ttthreads.set(b.threads);
                }
                Err(e) => {
                    gloo::console::log!(format!("{e:?}"));
                    // redirect to 404 page
                    if let Some(n) = ttnav {
                        n.replace(&crate::BaseRoute::NotFound);
                    }
                }
            }
            ttloadingthreads.set(false);
        });
    });

    let firstrun = use_state(|| true);
    if *firstrun {
        load_threads.emit(());
        firstrun.set(false);
    }

    let post_content = use_state(String::new);

    html! {
        <div class="board">
            <div class="meta-shiz">
                <NavBar board_discriminator={props.board_discriminator.clone()}/>
                <BoardTitle board_discriminator={props.board_discriminator.clone()}/>
                <Banner board_discriminator={props.board_discriminator.clone()} />
                <div class="postbox">
                    <PostBox board_discriminator={props.board_discriminator.clone()} post_text={post_content} />
                </div>
            </div>
            <div class="board-threads">
                {
                    for threads.iter().map(|t| {
                        html! {
                            <div class="board-thread">
                                <ThreadView thread={MaybeExpandableThread::from(t.clone())} board_discriminator={props.board_discriminator.clone()} rerender={false}/>
                            </div>
                        }
                    })
                }
            </div>
            <div class="footer">
                <Banner board_discriminator={props.board_discriminator.clone()} />
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub board_discriminator: String,
}
