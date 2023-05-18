use common::structs::{SafePost, User};
use yew::prelude::*;
use yew_router::prelude::use_route;

use crate::{
    components::{DeleteButton, File, MaybeLink, Reply, RichText},
    helpers::CallbackContext,
    BaseRoute,
};

#[function_component]
pub fn Post(props: &Props) -> Html {
    let is_thread = use_route::<BaseRoute>()
        .map(|b| b.thread_id().is_some())
        .unwrap_or(false);

    let timezone = use_context::<UseStateHandle<chrono_tz::Tz>>();

    let on_click = use_state(|| None);
    let callback = use_context::<Option<CallbackContext>>().flatten();

    {
        let props = props.clone();
        let on_click = on_click.clone();
        use_effect_with_deps(
            move |callback| {
                if let Some(c) = callback {
                    let props = props.clone();
                    let c = c.clone();
                    on_click.set(Some(Callback::from(move |e: MouseEvent| {
                        e.prevent_default();
                        let reply = common::structs::Reply {
                            board_discriminator: props.post.board_discriminator.clone(),
                            post_number: props.post.post_number.to_string(),
                            thread_post_number: Some(props.post.thread_post_number.to_string()),
                            external: false,
                        };
                        c.callback.emit(reply);
                    })));
                }
            },
            callback,
        );
    }

    html! {
        <>
            {
                if props.topic.is_some() {
                    if let Some(ref file) = props.post.file {
                        html! {
                            <div class="left-file">
                                <div class="post-file">
                                    <File file={file.clone()} />
                                </div>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                } else {
                    html! {}
                }
            }
            <div class={ if props.topic.is_some() { "parent-post" } else { "post" } } id={ if props.invert { "invert" } else { "normal" }} >
                <div class="post-header">
                    <DeleteButton post_number={props.post.post_number} board_discriminator={props.post.board_discriminator.clone()} />
                    <span class="post-author">{ match &props.post.author {
                        User::Anonymous => { html! { <>{"Anonymous"}</> } }
                        User::Named(name) => { html! { <>{name}</> } }
                        User::Mod(name) => { html! { <span class="post-author-admin">{format!("{} ## MOD", name)}</span> } }
                    } }</span>
                    {
                        if let Some(on_click) = &*on_click {
                            html! {
                                <span class="post-number" onclick={on_click}>{ format!("#{}", props.post.post_number) }</span>
                            }
                        } else {
                            html! {
                                <span class="post-number">{ format!("#{}", props.post.post_number) }</span>
                            }
                        }
                    }
                    <span class="post-timestamp">{ if let Some(timezone) = timezone { props.post.timestamp.with_timezone(&*timezone).format("%Y-%m-%d %H:%M:%S %Z").to_string() } else { props.post.timestamp.format("%Y-%m-%d %H:%M:%S %Z").to_string() } }</span>
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
                {
                    if props.topic.is_none() || !props.post.content.is_empty() {
                        html! {
                            <div class="post-body">
                                {
                                    if props.topic.is_none() {
                                        if let Some(ref file) = props.post.file {
                                            html! {
                                                <div class="post-file">
                                                    <File file={file.clone()} />
                                                </div>
                                            }
                                        } else {
                                            html! {}
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
                         }
                    } else {
                        html! {}
                    }
                }
            </div>
        </>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    pub post: SafePost,
    #[prop_or_default]
    pub invert: bool,
    pub topic: Option<AttrValue>,
}
