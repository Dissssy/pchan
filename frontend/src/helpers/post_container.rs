use yew::prelude::*;

use crate::pages::board_page::SafePost;

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
                        {
                            match props.hyperlink {
                                Some(_) => {
                                    html! {
                                        <a href={format!("./{}", post.post_number)}>
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
                                            let mimetype = img.replace("/files/", "");
                                            let mime = mimetype.split('/').next();
                                            match mime {
                                                None => {
                                                    html! {
                                                        <div class="post-media-error">
                                                            <img src="/res/404.png"/>
                                                            <a href={img.clone()}>{"Unsupported embed type: None"}</a>
                                                        </div>
                                                    }
                                                }
                                                Some(m) => {
                                                    match m {
                                                        "video" => {
                                                            html! {
                                                                <video controls=true class="post-media-video">
                                                                    <source src={img.clone()} />
                                                                </video>
                                                            }
                                                        }
                                                        "audio" => {
                                                            html! {
                                                                <audio controls=true class="post-media-audio">
                                                                    <source src={img.clone()} />
                                                                </audio>
                                                            }
                                                        }
                                                        "image" => {
                                                            html! {
                                                                <img src={img.clone()} />
                                                            }
                                                        }
                                                        _ => {
                                                            html! {
                                                                <div class="post-media-error">
                                                                    <img src="/res/404.png"/>
                                                                    <a href={img.clone()}>{"Unsupported embed type: "}{m}</a>
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
                                                <img src={format!("{img}-thumb.jpg")} />
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
    pub hyperlink: Option<()>,
}
