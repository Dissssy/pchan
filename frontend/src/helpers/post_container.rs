use common::structs::SafePost;
use yew::prelude::*;

use super::delete_button::DeleteButton;

use crate::helpers::{
    lazy_post::LazyPost, possibly_long_text::TruncateText, HoveredOrExpandedState,
};

#[function_component]
pub fn PostView(props: &PostViewProps) -> Html {
    let _prevent_click = Callback::from(|e: MouseEvent| e.prevent_default());

    let add_to = props.add_to_content.clone();
    let id = props.post.post_number;
    let on_click_add = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        if let Some(ref add_to) = add_to {
            let prior = (*add_to).clone();
            let prior = if prior.trim().is_empty() {
                "".to_string()
            } else {
                format!("{}\n", &*prior)
            };
            add_to.set(format!("{}>>{}\n", prior, id));
        }
    });

    let file_state = use_state(|| HoveredOrExpandedState::None);

    let tfile_state = file_state.clone();
    let on_click = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        tfile_state.set(match *tfile_state {
            HoveredOrExpandedState::None => HoveredOrExpandedState::Expanded,
            HoveredOrExpandedState::Hovered => HoveredOrExpandedState::Expanded,
            HoveredOrExpandedState::Expanded => HoveredOrExpandedState::None,
        });
    });

    let tfile_state = file_state.clone();
    let mvprops = props.clone();
    let on_mouseon = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        if if let Some(ref f) = mvprops.post.file {
            !f.spoiler
        } else {
            true
        } {
            tfile_state.set(match *tfile_state {
                HoveredOrExpandedState::None => HoveredOrExpandedState::Hovered,
                HoveredOrExpandedState::Hovered => HoveredOrExpandedState::Hovered,
                HoveredOrExpandedState::Expanded => HoveredOrExpandedState::Expanded,
            });
        }
    });

    let tfile_state = file_state.clone();
    let on_mouseoff = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        tfile_state.set(match *tfile_state {
            HoveredOrExpandedState::None => HoveredOrExpandedState::None,
            HoveredOrExpandedState::Hovered => HoveredOrExpandedState::None,
            HoveredOrExpandedState::Expanded => HoveredOrExpandedState::Expanded,
        });
    });

    let invert = props.invert.unwrap_or(false);

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
                    <DeleteButton post_number={post.post_number} board_discriminator={props.board_discrim.clone()} load_posts={props.load_posts.clone()} />
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
                            match props.add_to_content {
                                Some(_) => {
                                    html! {
                                        <a href="#" onclick={on_click_add}>
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
                        if let Some(ref t) = props.topic {
                            html! {
                                <div class="post-header-topic">
                                    <a class="post-topic-link" href={format!("/{}/thread/{}", props.board_discrim, post.post_number)}>
                                        {t.clone()}
                                    </a>
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }
                    {
                        if !post.replies.is_empty() {
                            html! {
                                <div class="post-header-replies">
                                    <div class="post-header-reply-list">
                                        <>{"Replies: "}</>
                                        {
                                            for post.replies.iter().map(|r| {
                                                html! {
                                                    <div class="post-header-reply-text">
                                                        <LazyPost reply={r.clone()} this_board={props.board_discrim.clone()} invert={invert} this_thread_post_number={props.this_thread_post_number} load_posts={props.load_posts.clone()} />
                                                    </div>
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
                                    <a href="#" onclick={on_click.clone() }>
                                        {
                                            if !(*file_state == HoveredOrExpandedState::None)  {
                                                format!("[-]{}", if *file_state == HoveredOrExpandedState::Expanded { " (held)" } else { "" })
                                            } else {
                                                "[+]".to_owned()
                                            }
                                        }
                                    </a>
                                    <span class="post-hash" title={img.hash.clone()}>
                                        {"#"}
                                    </span>
                                </div>
                                <div class="post-file">
                                    <a href={img.path.clone()} onclick={on_click} onmouseover={on_mouseon} onmouseleave={on_mouseoff} >
                                    {
                                        if !(*file_state == HoveredOrExpandedState::None) {
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
                                                                <video autoplay=true loop=true controls=true class="post-media-video">
                                                                    <source src={img.path.clone()} />
                                                                </video>
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
                                        } else {
                                            html! {
                                                <img src={img.thumbnail.clone()} />
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
                {
                    if !post.content.trim().is_empty() {
                        html! {
                            <div class="post-text">
                                <TruncateText text={post.content.clone()} invert={invert} this_thread_post_number={props.this_thread_post_number} load_posts={props.load_posts.clone()} board_discrim={props.board_discrim.clone()}/>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct PostViewProps {
    pub post: SafePost,
    pub add_to_content: Option<UseStateHandle<String>>,
    pub invert: Option<bool>,
    pub load_posts: Option<Callback<()>>,
    pub board_discrim: String,
    pub this_thread_post_number: i64,
    pub topic: Option<String>,
}
