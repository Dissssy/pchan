use serde::Deserialize;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::pages::{
    board_page::{BoardWithThreads, PostView, ThreadWithPosts},
    post_box::PostBox,
};

#[function_component]
pub fn ThreadPage(props: &Props) -> Html {
    // get reply value from query string
    let reply = match use_location().map(|l| l.query::<Reply>()) {
        Some(Ok(query)) => query.reply,
        Some(Err(e)) => {
            gloo::console::log!(format!("{e:?}"));
            None
        }
        None => None,
    };

    let thread = use_state(|| None);
    let nav = use_navigator();
    {
        let threads = thread.clone();
        let props = props.clone();
        use_effect_with_deps(
            move |_| {
                wasm_bindgen_futures::spawn_local(async move {
                    let fetch = gloo_net::http::Request::get(&format!(
                        "/api/v1/board/{}/{}",
                        props.board_discriminator, props.thread_id
                    ))
                    .send()
                    .await;
                    match fetch {
                        Ok(f) => match f.json::<ThreadWithPosts>().await {
                            Ok(thread) => {
                                threads.set(Some(thread));
                            }
                            Err(e) => {
                                gloo::console::log!(format!("{e:?}"));
                                // redirect to 404 page
                                match nav {
                                    Some(n) => n.push(&crate::BaseRoute::NotFound),
                                    None => {}
                                }
                            }
                        },
                        Err(e) => {
                            gloo::console::log!(format!("{e:?}"));
                        }
                    }
                });
                || {}
            },
            (),
        );
    }

    html! {
        <div class="threadposts">
            <div class="board-title">
                <BoardTitle board_discriminator={props.board_discriminator.clone()}/>
            </div>
            <div class="postbox">
                <PostBox board_discriminator={props.board_discriminator.clone()} thread_id={props.thread_id.clone()} starter_text={reply} />
            </div>
                {
                    match *thread {
                        Some(ref t) => {
                            html! {
                                <div class="threadposts-list">

                                <div class="threadposts-post">
                                    <PostView post={t.thread_post.clone()} />
                                </div>
                                    <div class="threadposts-replies">
                                        {
                                            for t.posts.iter().map(|p| {
                                                html! {
                                                    <div class="threadposts-post">
                                                        <PostView post={p.clone()} />
                                                    </div>
                                                }
                                            })
                                        }
                                    </div>
                                </div>
                            }
                        }
                        None => {
                            html! {
                                <div class="threadposts-post">
                                    <p>{"Loading..."}</p>
                                </div>
                            }
                        }
                    }
                }
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub board_discriminator: String,
    pub thread_id: String,
}

#[derive(Deserialize, Clone, PartialEq)]
pub struct Reply {
    reply: Option<String>,
}

#[function_component]
fn BoardTitle(props: &TitleProps) -> Html {
    let board_info = use_state(|| None);

    {
        let board_info = board_info.clone();
        let props = props.clone();
        use_effect_with_deps(
            move |_| {
                wasm_bindgen_futures::spawn_local(async move {
                    let fetch = gloo_net::http::Request::get(&format!(
                        "/api/v1/board/{}",
                        props.board_discriminator
                    ))
                    .send()
                    .await;
                    match fetch {
                        Ok(f) => match f.json::<BoardWithThreads>().await {
                            Ok(boardses) => {
                                board_info.set(Some(boardses.name));
                            }
                            Err(e) => {
                                gloo::console::log!(format!("{e:?}"));
                            }
                        },
                        Err(e) => {
                            gloo::console::log!(format!("{e:?}"));
                        }
                    }
                });
                || {}
            },
            (),
        );
    }

    html! {
        <div class="board-title">
            <h1>
                {
                    match *board_info {
                        Some(ref b) => {
                            html! {
                                <>{format!("/{}/ - {}", props.board_discriminator, b)}</>
                            }
                        }
                        None => {
                            html! {
                                <>{""}</>
                            }
                        }
                    }
                }
            </h1>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
struct TitleProps {
    pub board_discriminator: String,
}
