use serde::Deserialize;
use yew::prelude::*;

use crate::pages::post_box::PostBox;

#[function_component]
pub fn BoardPage(props: &Props) -> Html {
    let threads = use_state(Vec::new);

    {
        let threads = threads.clone();
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
                                threads.set(boardses.threads);
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
        <div class="board">
            <div class="board-title">
                <h1>{&props.board_discriminator}</h1>
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
                                    <PostView post={t.thread_post.clone()} />
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

#[function_component]
pub fn PostView(props: &PostViewProps) -> Html {
    let image_expanded = use_state(|| false);
    let glimage_glexpanded = image_expanded.clone();
    let onclick = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        gloo::console::log!("clicked");
        glimage_glexpanded.set(!*glimage_glexpanded);
    });

    // TODO: make clicking the post number put you in the thread with ?reply=>>{post_number}

    let mut last_empty = false;
    let mut first = true;
    let post = &props.post;
    html! {
            <div class="post-content">
                <div class="post-header">
                    // contains author name, post number, timestamp, and any replies
                    <div class="post-header-author">
                        {
                            if let Some(ref author) = post.author {
                                author
                            } else {
                                "Anonymous"
                            }
                        }
                    </div>
                    <div class="post-header-number">
                        {format!("No. {}", post.post_number)}
                    </div>
                    <div class="post-header-timestamp">
                        {post.timestamp.clone()}
                    </div>
                    <div class="post-header-replies">
                        {
                            if !post.replies.is_empty() {
                                html! {
                                    <div class="post-header-reply-list">
                                        <>{"Replies: "}</>
                                        {
                                            for post.replies.iter().map(|r| {
                                                html! {
                                                    <div class="post-header-reply-text">
                                                        {format!(">>{r}")}
                                                    </div>
                                                }
                                            })
                                        }
                                    </div>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                </div>
                {
                    if let Some(ref img) = post.image {
                        html! {
                            <div class="post-image-container">
                                <a href="#" onclick={onclick}>
                                    {
                                        if *image_expanded {
                                            "[-]"
                                        } else {
                                            "[+]"
                                        }
                                    }
                                </a>
                                <div class="post-image">
                                    {
                                        if *image_expanded {
                                            // turn "/files/video/webm/gfj51HYQyWHB_wAh.webm-thumb.jpg" into "video/webm" by replacing "/files/" with "" and then splitting on "/" then taking the first two elements and joining them with "/"
                                            let mimetype = img.replace("/files/", "").split('/').take(2).collect::<Vec<&str>>().join("/");
                                            html! {
                                                <embed src={img.clone()} type={mimetype} />
                                            }
                                        } else {
                                            html! {
                                                <embed src={format!("{img}-thumb.jpg")} type="image/jpg" />
                                            }
                                        }
                                    }
                                </div>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
                <div class="post-text">
                    {
                        for post.content.lines().map(|l| {
                            if l.is_empty() && !last_empty {
                                last_empty = true;
                                html! {
                                    <>
                                        <br />
                                    </>
                                }
                            } else if l.is_empty() {
                                last_empty = true;
                                html! {}
                            } else {
                                last_empty = false;
                                html! {
                                    <>
                                        {
                                            if first {
                                                first = false;
                                                html! {
                                                    <>
                                                        {l}
                                                    </>
                                                }
                                            } else {
                                                html! {
                                                    <>
                                                        <br />
                                                        {l}
                                                    </>
                                                }
                                            }
                                        }
                                    </>
                                }
                            }
                        })
                    }
                </div>
            </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct PostViewProps {
    pub post: SafePost,
}
