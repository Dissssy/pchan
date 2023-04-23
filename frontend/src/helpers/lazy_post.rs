use anyhow::{anyhow, Result};
use common::structs::SafePost;
use yew::prelude::*;

use crate::helpers::{post_container::PostView, HoveredOrExpandedState};

use common::structs::Reply;

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

    let post_state = use_state(|| HoveredOrExpandedState::None);

    let tpost_state = post_state.clone();
    let tpost = post.clone();
    let tload_post = load_post.clone();
    let on_click = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        tpost_state.set(match *tpost_state {
            HoveredOrExpandedState::None => HoveredOrExpandedState::Expanded,
            HoveredOrExpandedState::Hovered => HoveredOrExpandedState::Expanded,
            HoveredOrExpandedState::Expanded => HoveredOrExpandedState::Hovered,
        });
        // gloo::console::log!(format!("{:?}", *tpost_state));
        if tpost.is_none() {
            tload_post.emit(());
        }
    });

    let tpost_state = post_state.clone();
    let tpost = post.clone();
    let tload_post = load_post.clone();
    let on_mouseon = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        tpost_state.set(match *tpost_state {
            HoveredOrExpandedState::None => HoveredOrExpandedState::Hovered,
            HoveredOrExpandedState::Hovered => HoveredOrExpandedState::Hovered,
            HoveredOrExpandedState::Expanded => HoveredOrExpandedState::Expanded,
        });
        // gloo::console::log!(format!("{:?}", *tpost_state));
        if tpost.is_none() {
            tload_post.emit(());
        }
    });

    let tpost_state = post_state.clone();
    let tpost = post.clone();
    let on_mouseoff = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        tpost_state.set(match *tpost_state {
            HoveredOrExpandedState::None => HoveredOrExpandedState::None,
            HoveredOrExpandedState::Hovered => HoveredOrExpandedState::None,
            HoveredOrExpandedState::Expanded => HoveredOrExpandedState::Expanded,
        });
        // gloo::console::log!(format!("{:?}", *tpost_state));
        if tpost.is_none() {
            load_post.emit(());
        }
    });

    html! {
        <>
            <a href={props.reply.link()} class={
                match *post {
                    Some(Ok(_)) => "post-link",
                    Some(Err(_)) => "post-link-deleted",
                    None => "post-link-unloaded",
                }
            } onclick={on_click} onmouseover={on_mouseon} onmouseleave={on_mouseoff} >{format!("{}{}", props.reply.text(), if *post_state == HoveredOrExpandedState::Expanded { " (held)" } else { "" })}</a>
            {
                match *post_state {
                    HoveredOrExpandedState::None => {
                        html! {}
                    }
                    _ => {
                        html! {
                            <>
                                {
                                    match *post {
                                        Some(Ok(ref post)) => {
                                            html! {
                                                <div>
                                                    <PostView post={post.clone()} board_discrim={props.reply.board_discriminator.clone()} invert={!props.invert} />
                                                </div>
                                            }
                                        }
                                        Some(Err(ref e)) => {
                                            let eror = format!("{}", e);
                                            if eror.contains("Record not found") {
                                                html! {
                                                    <span>{" [deleted]"}</span>
                                                }
                                            } else {
                                                html! {
                                                    <div>{e.to_string()}</div>
                                                }
                                            }
                                        }
                                        None => {
                                            html! {
                                                <span>{"loading..."}</span>
                                            }
                                        }
                                    }
                                }
                            </>
                        }
                    }
                    // HoveredOrExpandedState::Hovered => {
                    //     html! {
                    //         <>
                    //             {
                    //                 match *post {
                    //                     Some(Ok(ref post)) => {
                    //                         html! {
                    //                             <div>
                    //                                 <PostView post={post.clone()} board_discrim={props.reply.board_discriminator.clone()} invert={!props.invert} />
                    //                             </div>
                    //                         }
                    //                     }
                    //                     Some(Err(ref e)) => {
                    //                         let eror = format!("{}", e);
                    //                         if eror.contains("Record not found") {
                    //                             html! {
                    //                                 <span>{" [deleted]"}</span>
                    //                             }
                    //                         } else {
                    //                             html! {
                    //                                 <div>{e.to_string()}</div>
                    //                             }
                    //                         }
                    //                     }
                    //                     None => {
                    //                         html! {
                    //                             <>{"loading..."}</>
                    //                         }
                    //                     }
                    //                 }
                    //             }
                    //         </>
                    //     }
                    // }
                    // _ => {
                    //     html! {}
                    // }
                }
            }
        </>
    }
}

// {
//                 if *post_HoveredOrExpandedState == HoveredOrExpandedState::Expanded {
// html! {
//     <>
//         <a href={props.reply.link()} onclick={on_click}>{props.reply.text()}</a>
//         {
//             match *post {
//                 Some(Ok(ref post)) => {
//                     html! {
//                         <PostView post={post.clone()} board_discrim={props.reply.board_discriminator.clone()} invert={!props.invert} />
//                     }
//                 }
//                 Some(Err(ref e)) => {
//                     html! {
//                         <div>{e.to_string()}</div>
//                     }
//                 }
//                 None => {
//                     html! {
//                         <div>{"loading..."}</div>
//                     }
//                 }
//             }
//         }
//     </>
// }

//                 } else {
//                     html! {
//                         <a href={props.reply.link()} onclick={on_click}>{props.reply.text()}</a>
//                     }
//                 }
//             }

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    pub reply: Reply,
    pub invert: bool,
    pub this_board: String,
}
