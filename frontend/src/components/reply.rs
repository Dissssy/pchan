use crate::{
    api::ApiState,
    components::{HoveredOrExpandedState, OffsetType, ParentOffset, Post},
    ApiContext, BaseRoute,
};
use yew::prelude::*;
use yew_router::prelude::use_route;

#[function_component]
pub fn Reply(props: &Props) -> Html {
    let api_ctx = use_context::<Option<ApiContext>>();
    let post = use_state(|| ApiState::Pending);
    let route = use_route::<BaseRoute>();

    let passdown_offset = use_state(|| None::<ParentOffset>);
    let parent_offset = use_context::<Option<ParentOffset>>()
        .flatten()
        .unwrap_or_default();
    // let screen_height = web_sys::window()
    //     .and_then(|w| w.inner_height().ok())
    //     .and_then(|h| h.as_f64())
    //     .unwrap_or(0.0);

    {
        let post = post.clone();
        let api_ctx = api_ctx;
        let reply = props.reply.clone();
        use_effect_with(route, move |_| {
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
                                .get_post(&reply.board_discriminator, &reply.post_number, false)
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
                        post.set(ApiState::ContextError(AttrValue::from("ApiContext")));
                    }
                }
            });
        });
    }

    // let expanded = use_state(|| false);
    // let expand = {
    //     let expanded = expanded.clone();
    //     Callback::from(move |e: MouseEvent| {
    //         e.prevent_default();
    //         expanded.set(!*expanded);
    //     })
    // };

    let state = use_state(|| HoveredOrExpandedState::None);

    let on_click = {
        let state = state.clone();
        let passdown_offset = passdown_offset.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            state.set(match *state {
                HoveredOrExpandedState::Expanded { x, y, offset } => {
                    HoveredOrExpandedState::Hovered { x, y, offset }
                }
                _ => {
                    let raw_x = e.page_x();
                    let raw_y = e.page_y();

                    passdown_offset.set(Some(ParentOffset { x: raw_x, y: raw_y }));

                    let x = raw_x - parent_offset.x;
                    let y = raw_y - parent_offset.y;

                    // let base_pos = e.client_y() as f64;

                    // let normalized_pos = base_pos / screen_height;

                    // let offset = if normalized_pos < 0.33 {
                    //     OffsetType::Bottom
                    // } else if normalized_pos > 0.66 {
                    //     OffsetType::Top
                    // } else {
                    //     OffsetType::Center
                    // };

                    HoveredOrExpandedState::Expanded {
                        x,
                        y,
                        offset: OffsetType::Bottom,
                    }
                }
            });
        })
    };

    let on_hover = {
        let state = state.clone();
        let passdown_offset = passdown_offset.clone();
        Callback::from(move |e: MouseEvent| {
            state.set(match *state {
                HoveredOrExpandedState::Expanded { x, y, offset } => {
                    HoveredOrExpandedState::Expanded { x, y, offset }
                }
                _ => {
                    let raw_x = e.page_x();
                    let raw_y = e.page_y();

                    passdown_offset.set(Some(ParentOffset { x: raw_x, y: raw_y }));

                    let x = raw_x - parent_offset.x;
                    let y = raw_y - parent_offset.y;

                    // let base_pos = e.client_y() as f64;

                    // let normalized_pos = base_pos / screen_height;

                    // let offset = if normalized_pos < 0.33 {
                    //     OffsetType::Bottom
                    // } else if normalized_pos > 0.66 {
                    //     OffsetType::Top
                    // } else {
                    //     OffsetType::Center
                    // };

                    HoveredOrExpandedState::Hovered {
                        x,
                        y,
                        offset: OffsetType::Bottom,
                    }
                }
            });
        })
    };

    let on_mouseoff = {
        let state = state.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            state.set(match *state {
                HoveredOrExpandedState::Expanded { x, y, offset } => {
                    HoveredOrExpandedState::Expanded { x, y, offset }
                }
                _ => HoveredOrExpandedState::None,
            });
        })
    };

    match post.standard_html("Reply", |post| {
        html! {
            <ContextProvider<Option<ParentOffset>> context={*passdown_offset}>
                <a href={format!("/{}/thread/{}", props.reply.board_discriminator, post.thread_post_number)} onclick={on_click.clone()} onmousemove={on_hover.clone()} onmouseleave={on_mouseoff.clone()}>
                    <span class="post-reply">{props.reply.text(props.thread_post_number.to_string())}</span>
                </a>
                {
                    if *state != HoveredOrExpandedState::None {
                        html! {
                            <div class="expanded-reply" style={
                                match *state {
                                    HoveredOrExpandedState::Expanded { x, y, offset } => {
                                        format!("left: calc({}px{}) !important; top: calc({}px) !important; position: absolute !important; transform: translateY({}) !important;", x, if parent_offset.x == 0 { " + 1em" } else { "" }, y, offset.percent())
                                    },
                                    HoveredOrExpandedState::Hovered { x, y, offset } => {
                                        format!("left: calc({}px{}) !important; top: calc({}px) !important; position: absolute !important; transform: translateY({}) !important;", x, if parent_offset.x == 0 { " + 1em" } else { "" }, y, offset.percent())
                                    },
                                    HoveredOrExpandedState::None => {
                                        String::new()
                                    }
                                }
                            }>
                                <Post post={post.clone()} invert={!props.invert} />
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
            </ContextProvider<Option<ParentOffset>>>
        }
    }) {
        Ok(h) => h,
        Err(_) => {
            html! {
                <span class="dead-link">{props.reply.text(props.thread_post_number.to_string())}</span>
            }
        }
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    pub reply: common::structs::Reply,
    pub thread_post_number: AttrValue,
    pub invert: bool,
}
