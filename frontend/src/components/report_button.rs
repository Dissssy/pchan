use yew::prelude::*;

use crate::{api::ApiError, ApiContext};

/// Flags a post for moderator attention.
///
/// Available to any member. Nothing about who reported is sent or stored, so this cannot be
/// held against the reporter later.
#[function_component]
pub fn ReportButton(props: &Props) -> Html {
    let state = use_state(|| ReportState::Idle);
    let api_ctx = use_context::<Option<ApiContext>>().flatten();

    let on_click = {
        let state = state.clone();
        let props = props.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            if !matches!(*state, ReportState::Idle) {
                return;
            }

            // A reason is required, so prompt rather than firing a bare report.
            let reason = web_sys::window()
                .and_then(|w| w.prompt_with_message("What is wrong with this post?").ok())
                .flatten()
                .unwrap_or_default();
            if reason.trim().is_empty() {
                return;
            }

            state.set(ReportState::Pending);
            let state = state.clone();
            let api_ctx = api_ctx.clone();
            let board = props.board_discriminator.clone();
            let post_number = props.post_number;
            wasm_bindgen_futures::spawn_local(async move {
                let res = match api_ctx.as_ref().and_then(|c| c.api.as_ref().ok()) {
                    Some(api) => api.report_post(&board, post_number, reason).await,
                    None => Err(ApiError::Other(AttrValue::Static("No API context"))),
                };
                match res {
                    Ok(msg) => state.set(ReportState::Done(msg)),
                    Err(e) => state.set(ReportState::Failed(format!("{}", *e))),
                }
            });
        })
    };

    html! {
        <div class="post-header-report-button">
            {
                match &*state {
                    ReportState::Idle => html! {
                        <a onclick={on_click} title="Report this post">{"⚑"}</a>
                    },
                    ReportState::Pending => html! { <span>{"…"}</span> },
                    ReportState::Done(msg) => html! { <span title={msg.clone()}>{"⚑✓"}</span> },
                    ReportState::Failed(err) => html! { <span title={err.clone()}>{"⚑✗"}</span> },
                }
            }
        </div>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    pub post_number: i64,
    pub board_discriminator: String,
}

enum ReportState {
    Idle,
    Pending,
    Done(String),
    Failed(String),
}
