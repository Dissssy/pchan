use common::structs::SafePost;
use yew::prelude::*;
use yew_router::prelude::use_route;

use crate::{
    components::{File, MaybeLink, Reply, RichText},
    BaseRoute,
};

#[function_component]
pub fn Post(props: &Props) -> Html {
    let is_thread = use_route::<BaseRoute>()
        .map(|b| b.thread_id().is_some())
        .unwrap_or(false);

    html! {
        <div class="post" id={ if props.invert { "invert" } else { "normal" }} >
            <div class="post-header">
                <span class="post-author">{ props.post.author.clone().unwrap_or("Anonymous".to_string()) }</span>
                <span class="post-number">{ format!("#{}", props.post.post_number) }</span>
                <span class="post-timestamp">{ props.post.timestamp.clone() }</span>
                {
                    if let Some(ref t) = props.topic {
                        html! {
                            <MaybeLink to={ BaseRoute::ThreadPage { board_discriminator: props.post.board_discriminator.clone(), thread_id: props.post.thread_post_number.to_string() }} link={!is_thread}>
                                <span class="post-topic">{t.clone()}</span>
                            </MaybeLink>
                        }
                    } else {
                        html! {

                        }
                    }
                }
                {
                    for props.post.replies.iter().map(|reply| {
                        html! {
                            <span class="post-header-reply">
                                <Reply reply={reply.clone()} thread_post_number={props.post.thread_post_number.to_string()} invert={props.invert} />
                            </span>
                        }
                    })
                }
            </div>
            <div class="post-body">
                {
                    if let Some(ref file) = props.post.file {
                        html! {
                            <div class="post-file">
                                <File file={file.clone()} />
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
                {
                    if !props.post.content.is_empty() {
                        html! {
                            <div class="post-content">
                                <RichText board={props.post.board_discriminator.clone()} content={props.post.content.clone()} thread_post_number={props.post.thread_post_number.to_string()} invert={props.invert} />
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
        </div>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    pub post: SafePost,
    #[prop_or_default]
    pub invert: bool,
    pub topic: Option<String>,
}
