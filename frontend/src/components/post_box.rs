use common::structs::{CreateFile, CreatePost, CreateThread};
use yew::prelude::*;
use yew_hooks::prelude::*;
use yew_router::prelude::*;

use crate::{
    api::{ApiError, ApiState},
    components::ContextError,
    ApiContext, BaseRoute,
};

#[function_component]
pub fn PostBox() -> Html {
    let possible_name = use_local_storage::<String>("name".to_string());
    let routeinfo =
        use_route::<BaseRoute>().map(|route| (route.board_discriminator(), route.thread_id()));
    let api_ctx = use_context::<Option<ApiContext>>().flatten();

    let show_box = use_state(|| false);

    let open_hovered = use_state(|| false);
    let close_hovered = use_state(|| false);
    let spoiler_hovered = use_state(|| false);

    let post = CreatePostInfo {
        name: use_state(|| possible_name.as_ref().unwrap_or(&"".to_string()).clone()),
        topic: use_state(|| "".to_string()),
        content: use_state(|| "".to_string()),
        file: use_state(|| None),
        spoiler: use_state(|| false),
    };

    let emojis = use_local_storage::<bool>("emojis".to_string()).unwrap_or(true);

    let state = use_state(|| ApiState::Pending);
    let on_click = {
        let routeinfo = routeinfo.clone();
        let post = post.clone();
        let state = state.clone();
        let api_ctx = api_ctx;
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            if *state == ApiState::Loading {
                gloo::console::error!("Already posting");
                return;
            }
            state.set(ApiState::Loading);
            let state = state.clone();
            let routeinfo = routeinfo.clone();
            let post = post.clone();
            let api_ctx = api_ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(api) = api_ctx.and_then(|ctx| ctx.api.ok()) {
                    let file = if let Some(file) = (*post.file).clone() {
                        match api.create_file(file).await {
                            Ok(file) => Some(CreateFile {
                                id: file,
                                spoiler: *post.spoiler,
                            }),
                            Err(e) => {
                                state.set(ApiState::Error(e));
                                return;
                            }
                        }
                    } else {
                        None
                    };
                    gloo::console::log!(format!("file: {:?}", file));

                    let create_post = CreatePost {
                        author: Some((*post.name).clone()).filter(|name| !name.is_empty()),
                        content: (*post.content).clone(),
                        file,
                    };

                    match match routeinfo {
                        None => {
                            state.set(ApiState::ContextError("BaseRoute".to_string()));
                            return;
                        }
                        Some((None, _)) => {
                            state.set(ApiState::ContextError(
                                "BaseRoute (board_discriminator)".to_string(),
                            ));
                            return;
                        }
                        Some((Some(board_discriminator), None)) => {
                            let create_thread = CreateThread {
                                post: create_post,
                                topic: (*post.topic).clone(),
                            };
                            api.create_thread(&board_discriminator, create_thread).await
                        }
                        Some((Some(board_discriminator), Some(thread_id))) => {
                            api.create_post(&board_discriminator, &thread_id, create_post)
                                .await
                        }
                    } {
                        Ok(v) => state.set(ApiState::Loaded(v)),
                        Err(e) => state.set(ApiState::Error(e)),
                    }
                } else {
                    state.set(ApiState::ContextError("ApiContext".to_string()));
                }
            });
        })
    };

    let on_input_name = post.name_change_callback(possible_name.clone());
    let on_input_topic = post.topic_change_callback();
    let on_input_content = post.content_change_callback();
    let on_change_file = post.file_change_callback();
    let on_click_spoiler = post.spoiler_change_callback();

    match routeinfo {
        Some((Some(_), thread)) => {
            if *show_box {
                html! {
                    <div class={ if thread.is_some() { "post-box-floating" } else { "post-box-centered" } }>
                        <div class="post-box">
                            <div class="post-box-meta">
                                <a href="#"
                                    onclick={ let show_box = show_box.clone(); Callback::from(move |_| show_box.set(!*show_box))}
                                    onmouseover={ let close_hovered = close_hovered.clone(); Callback::from(move |_| close_hovered.set(true)) }
                                    onmouseout={ let close_hovered = close_hovered.clone(); Callback::from(move |_| close_hovered.set(false)) }>
                                    { if *close_hovered { if emojis { "🔐" } else { "Cloes" } } else if emojis { "🔓" } else { "Close" } }
                                </a>
                                <div class="post-box-meta-inputs">
                                    <div class="post-box-name">
                                        <input type="text" placeholder="Anonymous" value={possible_name.as_ref().unwrap_or(&"".to_string()).clone()} oninput={on_input_name} />
                                    </div>
                                    {
                                        if thread.is_none() {
                                            html! {
                                                <div class="post-box-topic">
                                                    <input type="text" placeholder="Topic (Required)" value={(*post.topic).clone()} oninput={on_input_topic} />
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                            </div>
                            <div class="post-box-content">
                                <textarea value={(*post.content).clone()} placeholder={ format!("Content ({})",  if thread.is_some() { if post.file.is_none() { "Or File" } else { "Optional" } } else { "Optional" })} oninput={on_input_content} />
                            </div>
                            <div class="post-box-file">
                                <a href="#" title="spoiler" onclick={on_click_spoiler.clone()}
                                    onmouseover={ let spoiler_hovered = spoiler_hovered.clone(); Callback::from(move |_| spoiler_hovered.set(true)) }
                                    onmouseout={ let spoiler_hovered = spoiler_hovered.clone(); Callback::from(move |_| spoiler_hovered.set(false)) }
                                >{ match (*post.spoiler, *spoiler_hovered) {
                                    ( true,  true) => if emojis { "❎" } else { "UnSpoiler" }, // file is spoilered and also currently hovered
                                    ( true, false) => if emojis { "✅" } else { "UnSpoiler" }, // file is spoilered but not hovered
                                    (false,  true) => if emojis { "❎" } else { "Spoiler" }, // file is not spoilered but hovered
                                    (false, false) => if emojis { "🟩" } else { "Spoiler" }, // file is not spoilered and not hovered
                                } }</a>
                                <input type="file" onchange={on_change_file} />
                                <span>{format!("({})", if thread.is_some() { if post.content.is_empty() { "Or Content" } else { "Optional" } } else { "Required" })}</span>
                            </div>
                            <div class="post-box-submit">
                                <a href="#" onclick={on_click.clone()}>{ if thread.is_some() { "Reply" } else { "Create Thread" } }</a>
                                {
                                    match &*state {
                                        ApiState::Pending => html! {},
                                        ApiState::Loading => html! { <div class="post-box-loading"><span>{"Loading..."}</span></div> },
                                        ApiState::Loaded(_) => html! { <div class="post-box-success"><span>{"Success!"}</span></div> },
                                        ApiState::Error(ApiError::Api(e)) => html! { <div class="post-box-error"><span>{e}</span></div> },
                                        ApiState::Error(e) => {
                                            gloo::console::error!(format!("Error: {:?}", e));
                                            html! { <div class="post-box-error"><span>{"Unknown Error! Check console for details."}</span></div> }
                                        }
                                        ApiState::ContextError(ref e) => html! { <div class="post-box-error"><span>{"Context Error: "}{e}</span></div> },
                                    }
                                }
                            </div>
                        </div>
                    </div>
                }
            } else {
                html! {
                    <div class="post-box-centered">
                        <div class="post-box">
                            <a href="#" onclick={Callback::from(move |_| show_box.set(!*show_box))}
                            onmouseover={ let open_hovered = open_hovered.clone(); Callback::from(move |_| open_hovered.set(true)) }
                            onmouseout={ let open_hovered = open_hovered.clone(); Callback::from(move |_| open_hovered.set(false)) }>
                                { if *open_hovered { if emojis { "🔑" } else { "Open" } } else if emojis { "🔒" } else { "Open" } }
                            </a>
                        </div>
                    </div>
                }
            }
        }
        _ => {
            html! {
                <ContextError cause={"BaseRoute"} source={"PostBox"}/>
            }
        }
    }
}

#[derive(Clone)]
pub struct CreatePostInfo {
    pub name: UseStateHandle<String>,
    pub topic: UseStateHandle<String>,
    pub content: UseStateHandle<String>,
    pub file: UseStateHandle<Option<web_sys::File>>,
    pub spoiler: UseStateHandle<bool>,
}

impl CreatePostInfo {
    pub fn name_change_callback(
        &self,
        name_storage: UseLocalStorageHandle<String>,
    ) -> Callback<InputEvent> {
        let name = self.name.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(value) = crate::helpers::on_input_to_string(e).map(|s| s.value()) {
                name_storage.set(value.clone());
                name.set(value);
            }
        })
    }

    pub fn topic_change_callback(&self) -> Callback<InputEvent> {
        let topic = self.topic.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(value) = crate::helpers::on_input_to_string(e).map(|s| s.value()) {
                topic.set(value);
            }
        })
    }

    pub fn content_change_callback(&self) -> Callback<InputEvent> {
        let content = self.content.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(value) = crate::helpers::on_input_textarea_to_string(e).map(|s| s.value()) {
                content.set(value);
            }
        })
    }

    pub fn file_change_callback(&self) -> Callback<Event> {
        let file = self.file.clone();
        Callback::from(move |e: Event| {
            if let Some(value) =
                crate::helpers::on_change_to_string(e).and_then(|s| s.files()?.get(0))
            {
                file.set(Some(value));
            }
        })
    }

    pub fn spoiler_change_callback(&self) -> Callback<MouseEvent> {
        let spoiler = self.spoiler.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            spoiler.set(!*spoiler);
        })
    }
}
