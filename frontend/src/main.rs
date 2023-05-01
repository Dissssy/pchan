pub mod api;
pub mod components;
pub mod helpers;
pub mod pages;
pub mod theme_data;

use gloo_storage::Storage;
use std::sync::Arc;
use theme_data::ThemeData;
use yew::prelude::*;
use yew_hooks::use_local_storage;
use yew_router::prelude::*;

use crate::components::settings::SettingsButton;

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

fn main() {
    yew::Renderer::<Root>::new().render();
}

#[function_component]
fn Root() -> Html {
    let theme_ctx = use_state(|| None);
    {
        let theme_ctx = theme_ctx.clone();
        use_effect(move || {
            if theme_ctx.is_none() {
                theme_ctx.set(Some(
                    gloo::storage::LocalStorage::get("theme")
                        .unwrap_or(ThemeData::default_dark_theme()),
                ));
            }
        });
    }

    let api_ctx = use_state(|| None);
    let dispatched = use_state(|| false);

    let token = use_local_storage::<String>("token".to_owned());

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
    match (&*api_ctx, &*theme_ctx) {
        (Some(api_ctx), Some(_)) => {
            html! {
                <ContextProvider<Option<ApiContext>> context={api_ctx.clone()}>
                    <ContextProvider<UseStateHandle<Option<ThemeData>>> context={theme_ctx.clone()}>
                        <BrowserRouter>
                            <SettingsButton/>
                            <Switch<BaseRoute> render={switch} />
                        </BrowserRouter>
                    </ContextProvider<UseStateHandle<Option<ThemeData>>>>
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
                <pages::home::Home/>
            }
        }
        BaseRoute::Settings => {
            html! {
                <pages::settings::Settings/>
            }
        }
        BaseRoute::BoardPage {
            board_discriminator,
        } => {
            html! {
                {board_discriminator}
                // <pages::board_page::BoardPage board_discriminator={board_discriminator} />
            }
        }
        BaseRoute::ThreadPage {
            board_discriminator,
            thread_id,
        } => {
            html! {
                <>
                {board_discriminator}
                {thread_id}
                </>
                // <pages::thread_page::ThreadPage board_discriminator={board_discriminator} thread_id={thread_id} />
            }
        }
        BaseRoute::NotFound => html! {
           {"404"}
        },
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct ApiContext {
    pub api: Result<Arc<api::Api>, api::ApiError>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct ThemeContext {
    pub theme: ThemeData,
    pub set_theme: Callback<ThemeData>,
}
