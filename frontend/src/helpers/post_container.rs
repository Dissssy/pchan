use common::structs::SafePost;
use yew::prelude::*;

use crate::helpers::{lazy_post::LazyPost, startswith_class::StartsWithClass, Reply};

#[function_component]
pub fn PostView(props: &PostViewProps) -> Html {
    let file_expanded = use_state(|| false);
    let glfile_glexpanded = file_expanded.clone();
    let onclick = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        gloo::console::log!("clicked");
        glfile_glexpanded.set(!*glfile_glexpanded);
    });

    // TODO: make clicking the post number put you in the thread with ?reply=>>{post_number}
    let invert = props.invert.unwrap_or(false);
    let mut last_empty = false;
    let mut first = true;
    let post = &props.post;
    html! {
            <div class={
                    if invert {
                        "post-content-invert"
                    } else {
                        "post-content"
                    }
                }>
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
                        {
                            match props.hyperlink {
                                Some(ref board_discrim) => {
                                    html! {
                                        <a href={format!("/{}/thread/{}", board_discrim, post.post_number)}>
                                            {format!("No. {}", post.post_number)}
                                        </a>
                                    }
                                }
                                None => {
                                    html! {
                                        <>
                                            {format!("No. {}", post.post_number)}
                                        </>
                                    }
                                }
                            }
                        }
                    </div>
                    <div class="post-header-timestamp">
                        {post.timestamp.clone()}
                    </div>
                        {
                            if !post.replies.is_empty() {
                                html! {
                                    <div class="post-header-replies">
                                        <div class="post-header-reply-list">
                                            <>{"Replies: "}</>
                                            {
                                                for post.replies.iter().map(|r| {
                                                    match Reply::from_str(&format!(">>{}", r), &props.board_discrim.clone()) {
                                                        Ok(r) => {
                                                            html! {
                                                                <div class="post-header-reply-text">
                                                                    <LazyPost reply={r} this_board={props.board_discrim.clone()} invert={invert} />
                                                                </div>
                                                            }
                                                        }
                                                        Err(_) => {
                                                            html! {
                                                                //<div class="post-header-reply-text">
                                                                //    {format!(">>{r}")}
                                                                //</div>
                                                            }
                                                        }
                                                    }
                                                })
                                            }
                                        </div>
                                    </div>
                                }
                            } else {
                                html! {}
                            }
                        }
                </div>
                {
                    if let Some(ref img) = post.file {
                        html! {
                            <div class="post-file-container">
                                <div class="post-file-header">
                                    <a href="#" onclick={onclick.clone()}>
                                        {
                                            if *file_expanded {
                                                "[-]"
                                            } else {
                                                "[+]"
                                            }
                                        }
                                    </a>
                                    <span class="post-hash">
                                        {"Hash: "}{img.hash.clone()}
                                    </span>
                                </div>
                                <div class="post-file">
                                    <a href={img.path.clone()} onclick={onclick}>
                                    {
                                        if *file_expanded {
                                            // turn "/files/video/webm/gfj51HYQyWHB_wAh.webm-thumb.jpg" into "video/webm" by replacing "/files/" with "" and then splitting on "/" then taking the first two elements and joining them with "/"
                                            let mimetype = img.path.replace("/files/", "");
                                            let mime = mimetype.split('/').next();
                                            match mime {
                                                None => {
                                                    html! {
                                                        <div class="post-media-error">
                                                            <img src="/res/404.png"/>
                                                            <a href={img.path.clone()}>{"Unsupported embed type: None"}</a>
                                                        </div>
                                                    }
                                                }
                                                Some(m) => {
                                                    match m {
                                                        "video" => {
                                                            html! {
                                                                <video controls=true class="post-media-video">
                                                                    <source src={img.path.clone()} />
                                                                </video>
                                                            }
                                                        }
                                                        "audio" => {
                                                            html! {
                                                                <audio controls=true class="post-media-audio">
                                                                    <source src={img.path.clone()} />
                                                                </audio>
                                                            }
                                                        }
                                                        "image" => {
                                                            html! {
                                                                <img src={img.path.clone()} />
                                                            }
                                                        }
                                                        _ => {
                                                            html! {
                                                                <div class="post-media-error">
                                                                    <img src="/res/404.png"/>
                                                                    <a href={img.path.clone()}>{"Unsupported embed type: "}{m}</a>
                                                                </div>
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            // html! {
                                            //     <embed src={img.clone()} type={mimetype} />
                                            // }
                                        } else {
                                            html! {
                                                //<a href="#" onclick={onclick}>
                                                    <img src={img.thumbnail.clone()} />
                                                //</a>
                                            }
                                        }
                                    }
                                    </a>
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
                                            if !first {
                                                html! {
                                                    <br />
                                                }
                                            } else {
                                                first = false;
                                                html! {}
                                            }
                                        }
                                        {
                                            if let Ok(r) = Reply::from_str(l, &props.board_discrim) {
                                                html! {
                                                    <LazyPost reply={r} this_board={props.board_discrim.clone()} invert={invert} />
                                                }
                                            } else {
                                                html! {
                                                    <StartsWithClass text={l.to_owned()} map={crate::CLASSMAP.clone()} />
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
    pub hyperlink: Option<String>,
    pub invert: Option<bool>,
    pub board_discrim: String,
}
