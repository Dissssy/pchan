pub mod api;
pub mod components;
pub mod helpers;
pub mod pages;

use std::sync::Arc;
use yew::prelude::*;
use yew_hooks::{use_local_storage, UseLocalStorageHandle};
use yew_router::prelude::*;

use crate::components::{FeedbackButton, SettingsButton};

#[derive(Clone, Routable, PartialEq, Debug)]
pub enum BaseRoute {
    #[at("/")]
    Home,
    #[at("/settings")]
    Settings,
    #[at("/:board_discriminator/")]
    BoardPage { board_discriminator: String },
    #[at("/:board_discriminator/thread/:thread_id")]
    ThreadPage {
        board_discriminator: String,
        thread_id: String,
    },
    #[not_found]
    #[at("/404")]
    NotFound,
}

impl BaseRoute {
    pub fn board_discriminator(&self) -> Option<String> {
        match self {
            BaseRoute::BoardPage {
                board_discriminator,
            } => Some(board_discriminator.clone()),
            BaseRoute::ThreadPage {
                board_discriminator,
                ..
            } => Some(board_discriminator.clone()),
            _ => None,
        }
    }

    pub fn thread_id(&self) -> Option<String> {
        match self {
            BaseRoute::ThreadPage { thread_id, .. } => Some(thread_id.clone()),
            _ => None,
        }
    }
}

impl From<common::structs::Reply> for BaseRoute {
    fn from(reply: common::structs::Reply) -> Self {
        BaseRoute::ThreadPage {
            board_discriminator: reply.board_discriminator,
            thread_id: reply.post_number,
        }
    }
}

fn main() {
    yew::Renderer::<Root>::new().render();
}

#[function_component]
fn Root() -> Html {
    let api_ctx = use_state(|| None);
    let dispatched = use_state(|| false);

    let token = use_local_storage::<String>("token".to_owned());

    let theme_data = ThemeData {
        primary_color: use_local_storage::<String>("primary_color".to_owned()),
        secondary_color: use_local_storage::<String>("secondary_color".to_owned()),
        topic_color: use_local_storage::<String>("topic_color".to_owned()),
        bluetext_color: use_local_storage::<String>("bluetext_color".to_owned()),
        peetext_color: use_local_storage::<String>("peetext_color".to_owned()),
        border_color: use_local_storage::<String>("border_color".to_owned()),
        error_color: use_local_storage::<String>("error_color".to_owned()),
        text_color: use_local_storage::<String>("text_color".to_owned()),
        secondary_text_color: use_local_storage::<String>("secondary_text_color".to_owned()),
        border_width: use_local_storage::<String>("border_width".to_owned()),
        border_type: use_local_storage::<String>("border_type".to_owned()),
        link_color: use_local_storage::<String>("link_color".to_owned()),
        post_link_valid_color: use_local_storage::<String>("post_link_valid_color".to_owned()),
        post_link_unloaded_color: use_local_storage::<String>(
            "post_link_unloaded_color".to_owned(),
        ),
        post_link_invalid_color: use_local_storage::<String>("post_link_invalid_color".to_owned()),
        edge_padding: use_local_storage::<String>("edge_padding".to_owned()),
        animation_speed: use_local_storage::<String>("animation_speed".to_owned()),
        border_radius: use_local_storage::<String>("border_radius".to_owned()),
    };

    theme_data.ensure_set();

    {
        let api_ctx = api_ctx.clone();
        use_effect(move || {
            if !*dispatched {
                wasm_bindgen_futures::spawn_local(async move {
                    let api = api::Api::new(token).await.map(Arc::new);
                    api_ctx.set(Some(ApiContext { api }));
                });
                dispatched.set(true);
            }
        });
    }

    match &*api_ctx {
        Some(api_ctx) => {
            html! {
                <ContextProvider<Option<ApiContext>> context={api_ctx.clone()}>
                    <ContextProvider<ThemeData> context={theme_data}>
                        <BrowserRouter>
                            <SettingsButton/>
                            <FeedbackButton/>
                            <Switch<BaseRoute> render={switch} />
                        </BrowserRouter>
                    </ContextProvider<ThemeData>>
                </ContextProvider<Option<ApiContext>>>
            }
        }
        _ => html! {
            //<div class="valign">
            //    <div class="halign">
            //        <div class="loading">
            //            <h1>{"Loading..."}</h1>
            //        </div>
            //    </div>
            //</div>
        },
    }
}

fn switch(routes: BaseRoute) -> Html {
    match routes {
        BaseRoute::Home => {
            html! {
                <pages::Home/>
            }
        }
        BaseRoute::Settings => {
            html! {
                <pages::Settings/>
            }
        }
        BaseRoute::BoardPage {
            board_discriminator: _,
        } => {
            html! {
                <pages::BoardPage />
            }
        }
        BaseRoute::ThreadPage {
            board_discriminator: _,
            thread_id: _,
        } => {
            html! {
                <pages::ThreadPage />
            }
        }
        BaseRoute::NotFound => html! {
            <pages::NotFound />
        },
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct ApiContext {
    pub api: Result<Arc<api::Api>, api::ApiError>,
}

#[derive(Clone, PartialEq)]
pub struct ThemeData {
    pub primary_color: UseLocalStorageHandle<String>,
    pub secondary_color: UseLocalStorageHandle<String>,
    pub topic_color: UseLocalStorageHandle<String>,
    pub bluetext_color: UseLocalStorageHandle<String>,
    pub peetext_color: UseLocalStorageHandle<String>,
    pub border_color: UseLocalStorageHandle<String>,
    pub error_color: UseLocalStorageHandle<String>,
    pub text_color: UseLocalStorageHandle<String>,
    pub secondary_text_color: UseLocalStorageHandle<String>,
    pub border_width: UseLocalStorageHandle<String>,
    pub border_type: UseLocalStorageHandle<String>,
    pub link_color: UseLocalStorageHandle<String>,
    pub post_link_valid_color: UseLocalStorageHandle<String>,
    pub post_link_unloaded_color: UseLocalStorageHandle<String>,
    pub post_link_invalid_color: UseLocalStorageHandle<String>,
    pub edge_padding: UseLocalStorageHandle<String>,
    pub animation_speed: UseLocalStorageHandle<String>,
    pub border_radius: UseLocalStorageHandle<String>,
}

impl ThemeData {
    pub fn css(&self) -> String {
        format!(
            "--primary-color: {}; --secondary-color: {}; --topic-color: {}; --bluetext-color: {}; --peetext-color: {}; --border-color: {}; --error-color: {}; --text-color: {}; --secondary-text-color: {}; --border-width: {}; --border-type: {}; --link-color: {}; --post-link-valid-color: {}; --post-link-unloaded-color: {}; --post-link-invalid-color: {}; --edge-padding: {}; --animation-speed: {}; --border-radius: {};",
            (*self.primary_color).clone().unwrap_or("#282a2e".to_string()),
            (*self.secondary_color).clone().unwrap_or("#1d1f21".to_string()),
            (*self.topic_color).clone().unwrap_or("lightblue".to_string()),
            (*self.bluetext_color).clone().unwrap_or("#a039ff".to_string()),
            (*self.peetext_color).clone().unwrap_or("#bebe33".to_string()),
            (*self.border_color).clone().unwrap_or("#242424".to_string()),
            (*self.error_color).clone().unwrap_or("purple".to_string()),
            (*self.text_color).clone().unwrap_or("#c5c8c6".to_string()),
            (*self.secondary_text_color).clone().unwrap_or("#40dba0".to_string()),
            (*self.border_width).clone().unwrap_or("0.05em".to_string()),
            (*self.border_type).clone().unwrap_or("solid".to_string()),
            (*self.link_color).clone().unwrap_or("#2c7d31".to_string()),
            (*self.post_link_valid_color).clone().unwrap_or("#2c7d31".to_string()),
            (*self.post_link_unloaded_color).clone().unwrap_or("#a039ff".to_string()),
            (*self.post_link_invalid_color).clone().unwrap_or("#e74c3c".to_string()),
            (*self.edge_padding).clone().unwrap_or("2%".to_string()),
            (*self.animation_speed).clone().unwrap_or("200ms".to_string()),
            (*self.border_radius).clone().unwrap_or("0.3em".to_string()),
        )
    }
    pub fn reset_dark(&self) {
        self.primary_color.set("#282a2e".to_string());
        self.secondary_color.set("#1d1f21".to_string());
        self.topic_color.set("lightblue".to_string());
        self.bluetext_color.set("#a039ff".to_string());
        self.peetext_color.set("#bebe33".to_string());
        self.border_color.set("#242424".to_string());
        self.error_color.set("purple".to_string());
        self.text_color.set("#c5c8c6".to_string());
        self.secondary_text_color.set("#40dba0".to_string());
        self.border_width.set("0.05em".to_string());
        self.border_type.set("solid".to_string());
        self.link_color.set("#2c7d31".to_string());
        self.post_link_valid_color.set("#2c7d31".to_string());
        self.post_link_unloaded_color.set("#a039ff".to_string());
        self.post_link_invalid_color.set("#e74c3c".to_string());
        self.edge_padding.set("2%".to_string());
        self.animation_speed.set("200ms".to_string());
        self.border_radius.set("0.3em".to_string());
    }
    pub fn reset_light(&self) {
        self.primary_color.set("#f0e0d6".to_string());
        self.secondary_color.set("#ffffee".to_string());
        self.topic_color.set("purple".to_string());
        self.bluetext_color.set("#a039ff".to_string());
        self.peetext_color.set("#bebe33".to_string());
        self.border_color.set("#d9bfb7".to_string());
        self.error_color.set("purple".to_string());
        self.text_color.set("maroon".to_string());
        self.secondary_text_color.set("#1c71d8".to_string());
        self.border_width.set("0.05em".to_string());
        self.border_type.set("solid".to_string());
        self.link_color.set("#986a44".to_string());
        self.post_link_valid_color.set("#2c7d31".to_string());
        self.post_link_unloaded_color.set("#a039ff".to_string());
        self.post_link_invalid_color.set("#e74c3c".to_string());
        self.edge_padding.set("2%".to_string());
        self.animation_speed.set("200ms".to_string());
        self.border_radius.set("0.3em".to_string());
    }
    pub fn ensure_set(&self) {
        if (*self.primary_color).is_none() {
            self.primary_color.set("#282a2e".to_string());
        }
        if (*self.secondary_color).is_none() {
            self.secondary_color.set("#1d1f21".to_string());
        }
        if (*self.topic_color).is_none() {
            self.topic_color.set("lightblue".to_string());
        }
        if (*self.bluetext_color).is_none() {
            self.bluetext_color.set("#a039ff".to_string());
        }
        if (*self.peetext_color).is_none() {
            self.peetext_color.set("#bebe33".to_string());
        }
        if (*self.border_color).is_none() {
            self.border_color.set("#242424".to_string());
        }
        if (*self.error_color).is_none() {
            self.error_color.set("purple".to_string());
        }
        if (*self.text_color).is_none() {
            self.text_color.set("#c5c8c6".to_string());
        }
        if (*self.secondary_text_color).is_none() {
            self.secondary_text_color.set("#40dba0".to_string());
        }
        if (*self.border_width).is_none() {
            self.border_width.set("0.05em".to_string());
        }
        if (*self.border_type).is_none() {
            self.border_type.set("solid".to_string());
        }
        if (*self.link_color).is_none() {
            self.link_color.set("#2c7d31".to_string());
        }
        if (*self.post_link_valid_color).is_none() {
            self.post_link_valid_color.set("#2c7d31".to_string());
        }
        if (*self.post_link_unloaded_color).is_none() {
            self.post_link_unloaded_color.set("#a039ff".to_string());
        }
        if (*self.post_link_invalid_color).is_none() {
            self.post_link_invalid_color.set("#e74c3c".to_string());
        }
        if (*self.edge_padding).is_none() {
            self.edge_padding.set("2%".to_string());
        }
        if (*self.animation_speed).is_none() {
            self.animation_speed.set("200ms".to_string());
        }
        if (*self.border_radius).is_none() {
            self.border_radius.set("0.3em".to_string());
        }
    }
}
