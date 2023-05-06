use crate::{api::ApiState, ApiContext, components::Post, BaseRoute};
use yew::prelude::*;
use yew_router::prelude::use_route;

#[function_component]
pub fn Reply(props: &Props) -> Html {
    let api_ctx = use_context::<Option<ApiContext>>();
    let post = use_state(|| ApiState::Pending);
    let route = use_route::<BaseRoute>();

    {
        let post = post.clone();
        let api_ctx = api_ctx;
        let reply = props.reply.clone();
        use_effect_with_deps(
            move |_| {
                post.set(ApiState::Loading);
                let reply = reply.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api_ctx {
                        Some(Some(api_ctx)) => match api_ctx.api {
                            Err(e) => {
                                post.set(ApiState::Error(e));
                            }
                            Ok(api) => {
                                match api
                                    .get_post(&reply.board_discriminator, &reply.post_number)
                                    .await
                                {
                                    Err(e) => {
                                        post.set(ApiState::Error(e));
                                    }
                                    Ok(thispost) => {
                                        post.set(ApiState::Loaded(thispost));
                                    }
                                };
                            }
                        },
                        _ => {
                            post.set(ApiState::ContextError("ApiContext".to_string()));
                        }
                    }
                });
            }, route
        );
    }

    let expanded = use_state(|| false);
    let expand = {
        let expanded = expanded.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            expanded.set(!*expanded);
        })
    };

    match post.standard_html("Reply", |post| {
        html! {
            <>
                <a href={format!("/{}/thread/{}", props.reply.board_discriminator, post.thread_post_number)} onclick={expand.clone()}>
                    <span class="post-reply">{props.reply.text(props.thread_post_number.clone())}</span>
                </a>
                {
                    if *expanded {
                        html! {
                            <div class="expanded-reply">
                                <Post post={post.clone()} invert={!props.invert} />
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
            </>
            
        }
    }) {
        Ok(h) => h,
        Err(e) => {
            html! {
                format!("Error: {:?}", e)
            }
        }
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    pub reply: common::structs::Reply,
    pub thread_post_number: String,
    pub invert: bool,
}
