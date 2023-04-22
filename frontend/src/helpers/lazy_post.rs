use anyhow::{anyhow, Result};
use common::structs::SafePost;
use yew::prelude::*;

use crate::helpers::post_container::PostView;

use super::Reply;

#[function_component]
pub fn LazyPost(props: &Props) -> Html {
    // TODO: make clickable, when clicked EMBED the post into the page awesome style

    let post = use_state(|| None::<Result<SafePost>>);
    let tpost = post.clone();
    let reply = props.reply.clone();
    let load_post = Callback::from(move |_: ()| {
        if tpost.is_none() {
            tpost.set(Some(Err(anyhow!("loading..."))));
            let post = tpost.clone();
            let reply = reply.clone();
            wasm_bindgen_futures::spawn_local(async move {
                post.set(Some(crate::API.lock().await.get_post(&reply).await));
            });
        }
    });

    let expanded = use_state(|| false);

    let texpanded = expanded.clone();
    let tpost = post.clone();
    let on_click = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        texpanded.set(!*texpanded);
        if tpost.is_none() {
            load_post.emit(());
        }
    });

    html! {
        <>
            <a href={props.reply.link()} onclick={on_click}>{props.reply.text()}</a>
            {
                if *expanded {
                    match *post {
                        Some(Ok(ref post)) => {
                            html! {
                                <PostView post={post.clone()} board_discrim={props.reply.board_discrim.clone()} invert={!props.invert} />
                            }
                        }
                        Some(Err(ref e)) => {
                            html! {
                                <div>{e.to_string()}</div>
                            }
                        }
                        None => {
                            html! {
                                <div>{"loading..."}</div>
                            }
                        }
                    }
                } else {
                    html! {}
                }
            }
        </>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    pub reply: Reply,
    pub invert: bool,
    pub this_board: String,
}
