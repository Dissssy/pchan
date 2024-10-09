use serde::Deserialize;
use yew::prelude::*;
use yew_hooks::use_local_storage;
use yew_router::hooks::{use_location, use_navigator, use_route};

use crate::{api::ApiState, helpers::on_input_to_string, ApiContext, BaseRoute};

// get the current board and if we're on one present a button to generate an invite link that will copy it to the clipboard
// also present two text boxes, one surrounded by a / on either side, the other with an input box for a code (placeholder: "Code")
// the first text box will be the board discriminator that the invite is being used for, the second will be the code that verifies the invite

#[function_component]
pub fn InviteTools(props: &Props) -> Html {
    let emojis = use_local_storage::<bool>("emojis".to_owned());
    // get the current board discriminator
    let location = use_route::<BaseRoute>();
    let board_discriminator = location.and_then(|b| b.board_discriminator());

    let navigator = use_navigator();

    let potential_code = use_location()
        .and_then(|l| l.query::<PotentialCode>().ok())
        .map(|p| p.invite_code);

    let code: UseStateHandle<String> = {
        let potential_code = potential_code.clone();
        use_state(|| potential_code.unwrap_or_default())
    };

    let state = use_state(|| ApiState::Pending::<()>);

    let api_ctx = use_context::<Option<ApiContext>>().flatten();

    let on_click_consume_code = {
        let state = state.clone();
        let code = code.clone();
        let api_ctx = api_ctx.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            match api_ctx {
                Some(ref api_ctx) => match api_ctx.api.clone() {
                    Ok(api) => {
                        let api = api;
                        let code = code.clone();
                        let navigator = navigator.clone();
                        let state = state.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            match api.consume_code(code.to_string()).await {
                                Ok(_) => {
                                    state.set(ApiState::Loaded(()));
                                    // redirect to the root page

                                    match navigator {
                                        Some(nav) => {
                                            nav.push(&BaseRoute::Home);
                                        }
                                        None => {
                                            gloo::console::error!("Failed to get navigator");
                                        }
                                    }
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
                },
                None => {
                    state.set(ApiState::ContextError(AttrValue::from("ApiContext")));
                }
            }
        })
    };

    let board_invite_state = use_state(|| ApiState::Pending::<String>);

    let board_invite_name = use_state(String::new);

    let on_click_generate_board_invite = {
        let state = board_invite_state.clone();
        let invite_name = board_invite_name.clone();
        let api_ctx = api_ctx.clone();
        Callback::from(move |(e, b): (MouseEvent, String)| {
            e.prevent_default();
            match api_ctx {
                Some(ref api_ctx) => match api_ctx.api.clone() {
                    Ok(api) => {
                        let api = api;
                        let invite_name = invite_name.clone();
                        let state = state.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            match api
                                .generate_board_invite_link(b.to_string(), invite_name.to_string())
                                .await
                            {
                                Ok(v) => {
                                    if let Some(window) = web_sys::window() {
                                        if let Some(clip) = window.navigator().clipboard() {
                                            let _ = clip.write_text(&format!(
                                                "{}/redeem?invite_code={}",
                                                env!("URL"),
                                                v
                                            ));
                                        }
                                    }
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
                },
                None => {
                    state.set(ApiState::ContextError(AttrValue::from("ApiContext")));
                }
            }
        })
    };

    let moderator_invite_state = use_state(|| ApiState::Pending::<String>);

    let moderator_invite_name = use_state(String::new);

    let on_click_generate_moderator_invite = {
        let state = moderator_invite_state.clone();
        let invite_name = moderator_invite_name.clone();
        let api_ctx = api_ctx.clone();
        Callback::from(move |(e, b): (MouseEvent, String)| {
            e.prevent_default();
            match api_ctx {
                Some(ref api_ctx) => match api_ctx.api.clone() {
                    Ok(api) => {
                        let api = api;
                        let invite_name = invite_name.clone();
                        let state = state.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            match api
                                .generate_moderator_invite_link(
                                    b.to_string(),
                                    invite_name.to_string(),
                                )
                                .await
                            {
                                Ok(v) => {
                                    if let Some(window) = web_sys::window() {
                                        if let Some(clip) = window.navigator().clipboard() {
                                            let _ = clip.write_text(&format!(
                                                "{}/redeem?invite_code={}",
                                                env!("URL"),
                                                v
                                            ));
                                        }
                                    }
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
                },
                None => {
                    state.set(ApiState::ContextError(AttrValue::from("ApiContext")));
                }
            }
        })
    };

    let invites_expanded = use_state(|| false);

    let click_invite = {
        let invites_expanded = invites_expanded.clone();
        Callback::from(move |_| {
            invites_expanded.set(!*invites_expanded);
        })
    };

    html!(
        // <div class={ if props.potential_code.is_some() { "invite-tools glowing-box" } else { "invite-tools" } }>
        <div class="invite-tools">
            <div class="invite-tools-content">
                <span>{"Invite Tools"}</span>
                {
                    if props.expandable {
                        html! {
                            <a onclick={click_invite} class="invite-tools-button">
                                {
                                    if *invites_expanded {
                                        if emojis.unwrap_or(true) {
                                            ""
                                        } else {
                                            "Expand"
                                        }
                                    } else {
                                        // i am not collapsing this.
                                        if emojis.unwrap_or(true) {
                                            ""
                                        } else {
                                            "Collapse"
                                        }
                                    }
                                }
                            </a>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
            {
                if *invites_expanded || !props.expandable {
                    html! {
                        <div class="invite-tools-container">
                            <div class="invite-tools-content">
                                <span class="invite-tools-title">{"Code"}</span>
                                <input
                                    type="text"
                                    placeholder="Invite Code"
                                    value={(*code).clone()}
                                    oninput={move |e: InputEvent| {
                                        if let Some(e) = on_input_to_string(e) {
                                            code.set(e.value());
                                        }
                                    }}
                                />
                                {
                                    match &*state {
                                        ApiState::Loaded(_) => {
                                            html! {
                                                <span title={"Invite accepted"} >{
                                                    if emojis.unwrap_or(true) {
                                                        ""
                                                    } else {
                                                        "Invite accepted"
                                                    }
                                                }</span>
                                            }
                                        }
                                        ApiState::Error(e) => {
                                            html! {
                                                <span title={
                                                    format!("Failed to accept invite: {}", **e)
                                                } >{
                                                    if emojis.unwrap_or(true) {
                                                        ""
                                                    } else {
                                                        "Error"
                                                    }
                                                }</span>
                                            }
                                        }
                                        _ => {
                                            html! {
                                                <a onclick={on_click_consume_code} class={ if potential_code.is_some() { "invite-tools-button glowing-text" } else { "invite-tools-button" } }>{ if emojis.unwrap_or(true) { "" } else { "Accept Invite" } }</a>
                                            }
                                        }
                                    }
                                }
                            </div>
                            {
                                match board_discriminator.as_ref() {
                                    Some(discrim) => {
                                        html! {
                                            <>
                                                <div class="invite-tools-content">
                                                    <span class="invite-tools-title">{"Generate Board Invite"}</span>
                                                    <input
                                                        type="text"
                                                        placeholder="Invite Name"
                                                        value={(*board_invite_name).clone()}
                                                        oninput={move |e: InputEvent| {
                                                            if let Some(e) = on_input_to_string(e) {
                                                                board_invite_name.set(e.value());
                                                            }
                                                        }}
                                                    />
                                                    {
                                                        match &*board_invite_state {
                                                            ApiState::Loaded(invite) => {
                                                                html! {
                                                                    <span title={"Invite copied to clipboard"} >{
                                                                        if emojis.unwrap_or(true) {
                                                                            ""
                                                                        } else {
                                                                            invite
                                                                        }
                                                                    }</span>
                                                                }
                                                            }
                                                            ApiState::Error(e) => {
                                                                html! {
                                                                    <span title={
                                                                        format!("Failed to generate invite: {}", **e)
                                                                    } >{
                                                                        if emojis.unwrap_or(true) {
                                                                            ""
                                                                        } else {
                                                                            "Error"
                                                                        }
                                                                    }</span>
                                                                }
                                                            }
                                                            _ => {
                                                                let discrim = discrim.clone();
                                                                html! {
                                                                    <a onclick={Callback::from(move |m: MouseEvent| {
                                                                        on_click_generate_board_invite.emit((m, discrim.to_string()));
                                                                    })} class="invite-tools-button">{ if emojis.unwrap_or(true) { "" } else { "Generate Invite" } }</a>
                                                                }
                                                            }
                                                        }
                                                    }
                                                </div>
                                                <div class="invite-tools-content">
                                                    <span class="invite-tools-title">{"Generate Moderator Invite"}</span>
                                                    <input
                                                        type="text"
                                                        placeholder="Invite Name"
                                                        value={(*moderator_invite_name).clone()}
                                                        oninput={move |e: InputEvent| {
                                                            if let Some(e) = on_input_to_string(e) {
                                                                moderator_invite_name.set(e.value());
                                                            }
                                                        }}
                                                    />
                                                    {
                                                        match &*moderator_invite_state {
                                                            ApiState::Loaded(invite) => {
                                                                html! {
                                                                    <span title={"Invite copied to clipboard"} >{
                                                                        if emojis.unwrap_or(true) {
                                                                            ""
                                                                        } else {
                                                                            invite
                                                                        }
                                                                    }</span>
                                                                }
                                                            }
                                                            ApiState::Error(e) => {
                                                                html! {
                                                                    <span title={
                                                                        format!("Failed to generate invite: {}", **e)
                                                                    } >{
                                                                        if emojis.unwrap_or(true) {
                                                                            ""
                                                                        } else {
                                                                            "Error"
                                                                        }
                                                                    }</span>
                                                                }
                                                            }
                                                            _ => {
                                                                let discrim = discrim.clone();
                                                                html! {
                                                                    <a onclick={Callback::from(move |m: MouseEvent| {
                                                                        on_click_generate_moderator_invite.emit((m, discrim.to_string()));
                                                                    })} class="invite-tools-button">{ if emojis.unwrap_or(true) { "" } else { "Generate Invite" } }</a>
                                                                }
                                                            }
                                                        }
                                                    }
                                                </div>
                                            </>
                                        }
                                    }
                                    None => {
                                        html! {}
                                    }
                                }
                            }
                        </div>
                    }
                } else {
                    html! {}
                }
            }
        </div>
    )
}

#[derive(Deserialize, Debug, Clone, PartialEq, Properties)]
pub struct PotentialCode {
    pub invite_code: String,
}

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub expandable: bool,
}
