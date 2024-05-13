use common::structs::{CreateFile, CreatePost, CreateThread, Reply};
use yew::prelude::*;
use yew_hooks::prelude::*;
use yew_router::prelude::*;

use crate::{
    api::{ApiError, ApiState},
    components::ContextError,
    helpers::{CallbackEmitterContext, SuccessfulPostContext},
    ApiContext, BaseRoute,
};

#[function_component]
pub fn PostBox(props: &Props) -> Html {
    let possible_name = use_local_storage::<String>("name".to_string());
    let possible_code = use_local_storage::<String>("code".to_string());
    let routeinfo = use_route::<BaseRoute>().map(|route| {
        (
            route.board_discriminator(),
            match route.thread_id() {
                Some(id) => ThreadType::RealThread(AttrValue::from(id)),
                None => match &props.override_thread {
                    Some(id) => ThreadType::FakeThread(id.clone()),
                    None => ThreadType::None,
                },
            },
        )
    });
    let api_ctx = use_context::<Option<ApiContext>>().flatten();

    let show_box = use_state(|| false);

    let open_hovered = use_state(|| false);
    let close_hovered = use_state(|| false);
    let spoiler_hovered = use_state(|| false);

    let post = CreatePostInfo {
        opened: show_box,
        name: use_state(|| return possible_name.as_ref().unwrap_or(&String::new()).clone()),
        code: use_state(|| return possible_code.as_ref().unwrap_or(&String::new()).clone()),
        topic: use_state(String::new),
        content: use_state(String::new),
        file: use_state(|| None),
        spoiler: use_state(|| false),
    };

    let emojis = use_local_storage::<bool>("emojis".to_string()).unwrap_or(true);

    let on_successful = use_context::<SuccessfulPostContext>();

    let state = use_state(|| ApiState::Pending);
    let on_click = {
        let routeinfo = routeinfo.clone();
        let post = post.clone();
        let state = state.clone();
        let on_successful = on_successful;
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
            let on_successful = on_successful.clone();
            let post = post.clone();
            let api_ctx = api_ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(api) = api_ctx.and_then(|ctx| ctx.api.ok()) {
                    let file = if let Some(file) = (*post.file).clone() {
                        match api.create_file(file).await {
                            Ok(file) => Some(CreateFile {
                                id: file.to_string(),
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

                    let create_post = CreatePost {
                        author: Some((*post.name).clone()).filter(|name| !name.is_empty()),
                        content: (*post.content).clone(),
                        code: Some((*post.code).clone()).filter(|code| !code.is_empty()),
                        file,
                    };

                    match match routeinfo.map(|(board, thread)| (board, thread.id())) {
                        None => {
                            state.set(ApiState::ContextError(AttrValue::from("BaseRoute")));
                            return;
                        }
                        Some((None, _)) => {
                            state.set(ApiState::ContextError(AttrValue::from(
                                "BaseRoute (board_discriminator)",
                            )));
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
                        Ok(v) => {
                            if let Some(c) = on_successful {
                                c.callback.emit(v.clone());
                                if let Err(e) = api
                                    .set_watching(
                                        &v.board_discriminator,
                                        v.thread_post_number,
                                        true,
                                    )
                                    .await
                                {
                                    gloo::console::error!(format!(
                                        "Failed to set watching: {}",
                                        *e
                                    ));
                                }
                            }
                            post.reset();
                            state.set(ApiState::Loaded(v))
                        }
                        Err(e) => state.set(ApiState::Error(e)),
                    }
                } else {
                    state.set(ApiState::ContextError(AttrValue::from("ApiContext")));
                }
            });
        })
    };

    let emitter = use_context::<CallbackEmitterContext>();
    {
        let post = post.clone();
        use_effect_with(post, move |post| {
            if let Some(emitter) = emitter {
                let post = post.clone();
                emitter.callback.emit(Callback::from(move |s: Reply| {
                    post.opened.set(true);
                    let content = post.content.clone();
                    let this_content = (*content).clone();
                    let reply_text = s.same_board_reply_text();
                    // if the content contains a line that is the same as the reply text, we remove it and return
                    let mut just_removed = false;
                    let new_content = content
                        .split('\n')
                        .filter(|line| {
                            if just_removed {
                                just_removed = false;
                                line != &""
                            } else if line == &reply_text {
                                just_removed = true;
                                false
                            } else {
                                true
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    if new_content != this_content {
                        content.set(new_content);
                        return;
                    }
                    // if the content is empty, we can just append the reply text to the content and return
                    if this_content.is_empty() {
                        content.set(format!("{}\n", reply_text));
                        return;
                    }
                    // otherwise, we need to add a newline and the reply text
                    content.set(format!("{}\n{}\n", this_content.trim_end(), reply_text));
                }));
            }
        });
    }

    let on_input_name = post.name_change_callback(possible_name.clone());
    let on_input_code = post.code_change_callback(possible_code.clone());
    let on_input_topic = post.topic_change_callback();
    let on_input_content = post.content_change_callback();
    let on_change_file = post.file_change_callback();
    let on_click_spoiler = post.spoiler_change_callback();

    match routeinfo {
        Some((Some(_), thread)) => {
            html! {
                <div class={thread.class().to_string()}>
                    <div class="post-box">
                        <div class="post-box-meta">
                            <a
                                onclick={ let opened = post.opened.clone(); Callback::from(move |e: MouseEvent| {e.prevent_default(); opened.set(!*opened)})}
                                onmouseover={ let close_hovered = close_hovered.clone(); Callback::from(move |_| close_hovered.set(true)) }
                                onmouseout={ let close_hovered = close_hovered.clone(); Callback::from(move |_| close_hovered.set(false)) }>
                                { if *post.opened { if *open_hovered { if emojis { "󱀈" } else { "Open" } } else if emojis { "󱀈" } else { "Open" } } else if *close_hovered { if emojis { "󱀉" } else { "Cloes" } } else if emojis { "󱀉" } else { "Close" } }
                            </a>
                            <div class="post-box-meta-inputs" style={ if *post.opened { "" } else { "display: none;" } }>
                                <div class="post-box-name" id={ if thread.is_some() { "notop" } else { "sloppytoppy" } }>
                                    <input type="text" placeholder="Anonymous" value={possible_name.as_ref().unwrap_or(&String::new()).clone()} oninput={on_input_name} />
                                </div>
                                <div class="post-box-code" id={ if thread.is_some() { "notop" } else { "sloppytoppy" } }>
                                    <input type="text" placeholder="secret code" value={post.upper_code_val()} oninput={on_input_code} />
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
                        <div class="post-box-content" style={ if *post.opened { "" } else { "display: none;" } }>
                            <textarea value={(*post.content).clone()} placeholder={ format!("Content ({})",  if thread.is_some() { if post.file.is_none() { "Or File" } else { "Optional" } } else { "Optional" })} oninput={on_input_content} />
                        </div>
                        <div class="post-box-file" style={ if *post.opened { "" } else { "display: none;" } }>
                            <a title="spoiler" onclick={on_click_spoiler.clone()}
                                onmouseover={ let spoiler_hovered = spoiler_hovered.clone(); Callback::from(move |_| spoiler_hovered.set(true)) }
                                onmouseout={ let spoiler_hovered = spoiler_hovered.clone(); Callback::from(move |_| spoiler_hovered.set(false)) }
                            >{ match (*post.spoiler, *spoiler_hovered) {
                                ( true,  true) => if emojis { "󰀨" } else { "UnSpoiler" }, // file is spoilered and also currently hovered
                                ( true, false) => if emojis { "󰀨" } else { "UnSpoiler" }, // file is spoilered but not hovered
                                (false,  true) => if emojis { "" } else { "Spoiler" }, // file is not spoilered but hovered
                                (false, false) => if emojis { "" } else { "Spoiler" }, // file is not spoilered and not hovered
                            } }</a>
                            <input type="file" onchange={on_change_file} />
                            <span>{format!("({})", if thread.is_some() { if post.content.is_empty() { "Or Content" } else { "Optional" } } else { "Required" })}</span>
                        </div>
                        <div class="post-box-submit" style={ if *post.opened { "" } else { "display: none;" } }>
                            <a onclick={on_click.clone()}>{ if thread.is_some() { "Reply" } else { "Create Thread" } }</a>
                            {
                                match &*state {
                                    ApiState::Pending => html! {},
                                    ApiState::Loading => html! { <div class="post-box-loading"><span>{"Loading..."}</span></div> },
                                    ApiState::Loaded(_) => html! { <div class="post-box-success"><span>{"Success!"}</span></div> },
                                    ApiState::Error(ApiError::Api(e)) => html! { <div class="post-box-error"><span>{e}</span></div> },
                                    ApiState::Error(e) => {
                                        gloo::console::error!(format!("Error: {}", **e));
                                        html! { <div class="post-box-error"><span>{"Unknown Error! Check console for details."}</span></div> }
                                    }
                                    ApiState::ContextError(ref e) => html! { <div class="post-box-error"><span>{"Context Error: "}{e}</span></div> },
                                }
                            }
                        </div>
                    </div>
                </div>
            }
        }
        _ => {
            html! {
                <ContextError cause={"BaseRoute"} source={"PostBox"}/>
            }
        }
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    pub override_thread: Option<AttrValue>,
}

#[derive(Clone, PartialEq)]
pub struct CreatePostInfo {
    pub opened: UseStateHandle<bool>,
    pub name: UseStateHandle<String>,
    pub code: UseStateHandle<String>,
    pub topic: UseStateHandle<String>,
    pub content: UseStateHandle<String>,
    pub file: UseStateHandle<Option<web_sys::File>>,
    pub spoiler: UseStateHandle<bool>,
}

impl CreatePostInfo {
    pub fn upper_code_val(&self) -> String {
        self.code.to_uppercase()
    }

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

    pub fn code_change_callback(
        &self,
        code_storage: UseLocalStorageHandle<String>,
    ) -> Callback<InputEvent> {
        let code = self.code.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(value) = crate::helpers::on_input_to_string(e).map(|s| s.value()) {
                code_storage.set(value.clone());
                code.set(value);
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

    pub fn reset(&self) {
        self.topic.set(String::new());
        self.content.set(String::new());
        self.file.set(None);
        self.spoiler.set(false);
    }
}

#[derive(Clone, PartialEq)]
enum ThreadType {
    RealThread(AttrValue),
    FakeThread(AttrValue),
    None,
}

impl ThreadType {
    pub fn id(&self) -> Option<AttrValue> {
        match self {
            Self::RealThread(id) => Some(id.clone()),
            Self::FakeThread(id) => Some(id.clone()),
            Self::None => None,
        }
    }
    pub fn is_some(&self) -> bool {
        match self {
            Self::RealThread(_) => true,
            Self::FakeThread(_) => true,
            Self::None => false,
        }
    }
    pub fn is_none(&self) -> bool {
        match self {
            Self::RealThread(_) => false,
            Self::FakeThread(_) => false,
            Self::None => true,
        }
    }
    pub fn class(&self) -> AttrValue {
        match self {
            Self::RealThread(_) => "post-box-floating",
            Self::FakeThread(_) => "post-box-inline",
            Self::None => "post-box-centered",
        }
        .into()
    }
}
