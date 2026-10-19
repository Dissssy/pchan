use yew::prelude::*;

use crate::{api::ApiError, ApiContext};

/// Bans the author of a post. Only rendered for site mods.
///
/// The moderator never learns who the author is: the server recomputes the per-post author
/// hash for each member, bans the match, and replies only with whether one was found.
///
/// Click twice to confirm, matching how deletion works on these posts.
#[function_component]
pub fn BanButton(props: &Props) -> Html {
    let state = use_state(|| BanState::Idle);
    let api_ctx = use_context::<Option<ApiContext>>().flatten();

    let on_click = {
        let state = state.clone();
        let props = props.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            match *state {
                BanState::Idle => {
                    state.set(BanState::Confirm);
                    return;
                }
                BanState::Confirm => {}
                _ => return,
            }

            state.set(BanState::Pending);
            let state = state.clone();
            let api_ctx = api_ctx.clone();
            let board = props.board_discriminator.clone();
            let post_number = props.post_number;
            wasm_bindgen_futures::spawn_local(async move {
                let res = match api_ctx.as_ref().and_then(|c| c.api.as_ref().ok()) {
                    // Board-scoped: the conservative default, and the only scope a board
                    // moderator is permitted anyway. Sitewide bans belong in the admin panel
                    // where the scope can be chosen deliberately.
                    Some(api) => api.ban_post_author(&board, post_number, false, None).await,
                    None => Err(ApiError::Other(AttrValue::Static("No API context"))),
                };
                match res {
                    Ok(msg) => state.set(BanState::Done(msg)),
                    Err(e) => state.set(BanState::Failed(format!("{}", *e))),
                }
            });
        })
    };

    let on_mouseout = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| {
            if matches!(*state, BanState::Confirm) {
                state.set(BanState::Idle);
            }
        })
    };

    html! {
        <div class="post-header-ban-button" onmouseout={on_mouseout}>
            {
                match &*state {
                    BanState::Idle => html! {
                        <a onclick={on_click} title="Ban this post's author from this board">{"⦸"}</a>
                    },
                    BanState::Confirm => html! {
                        <a onclick={on_click} title="Click again to ban them from this board">
                            {"⦸?"}
                        </a>
                    },
                    BanState::Pending => html! { <span>{"…"}</span> },
                    BanState::Done(msg) => html! { <span title={msg.clone()}>{"⦸✓"}</span> },
                    BanState::Failed(err) => html! { <span title={err.clone()}>{"⦸✗"}</span> },
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

enum BanState {
    Idle,
    Confirm,
    Pending,
    Done(String),
    Failed(String),
}
