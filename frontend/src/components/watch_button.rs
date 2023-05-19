use yew::prelude::*;
use yew_hooks::use_effect_once;

use crate::{
    api::{ApiState},
    ApiContext,
};

#[function_component]
pub fn WatchButton(props: &Props) -> Html {
    let state = use_state(|| ApiState::Pending::<bool>);

    let api_ctx = use_context::<Option<ApiContext>>().flatten();

    let on_click = {
        let state = state.clone();
        let props = props.clone();
        let api_ctx = api_ctx.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            match api_ctx {
                Some(ref api_ctx) => {
                    match api_ctx.api.clone() {
                        Ok(api) => {
                            let api = api.clone();
                            let props = props.clone();
                            let state = state.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                match api.set_watching(&props.board_discriminator, props.post_number, !state.get_or(false)).await {
                                    Ok(v) => {
                                        state.set(ApiState::Loaded(v));
                                    }
                                    Err(e) => {
                                        state.set(ApiState::Error(e));
                                    }
                                };
                            });
                        }
                        Err(e) => {
                            state.set(ApiState::Error(e));
                        }
                    }
                }
                None => {
                    state.set(ApiState::ContextError(AttrValue::from("ApiContext")));
                }
            }
        })
    };

    {
        let state = state.clone();
        let api_ctx = api_ctx;
        let props = props.clone();
        use_effect_once(move || {
            match api_ctx {
                Some(api_ctx) => {
                    match api_ctx.api {
                        Ok(api) => {
                            let api = api.clone();
                            let props = props.clone();
                            let state = state.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                match api.get_watching(&props.board_discriminator, props.post_number).await {
                                    Ok(v) => {
                                        state.set(ApiState::Loaded(v));
                                    }
                                    Err(e) => {
                                        state.set(ApiState::Error(e));
                                    }
                                };
                                
                            });
                        }
                        Err(e) => {
                            state.set(ApiState::Error(e));
                        }
                    }
                }
                None => {
                    state.set(ApiState::ContextError(AttrValue::from("ApiContext")));
                }
            }
            || {}
        })
    }

    match state.standard_html("WatchButton", |v| {
        html! {
            <div class="post-watch-button">
                <span onclick={on_click.clone()}>
                    {
                        if *v {
                            "󰛐"
                        } else {
                            ""
                        }
                    }
                </span>
            </div>
        }
    }) {
        Ok(v) => v,
        Err(_e) => html! {
            <div class="post-watch-button">
                <span class="dead-link">
                    {
                        // format!(" ({:?})", e)
                        ""
                    }
                </span>
            </div>
        },
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    pub post_number: i64,
    pub board_discriminator: String,
}