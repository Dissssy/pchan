pub mod api;
use anyhow::Result;
use api::Api;
use async_lock::Mutex;
use gloo_storage::Storage;
use std::sync::Arc;
use yew::prelude::*;
use yew_router::prelude::*;

lazy_static::lazy_static! {
    pub static ref CLASSMAP: Vec<(String, String)> = vec![
        (">".to_owned(), "bluetext".to_owned()),
        ("<".to_owned(), "peetext".to_owned())
    ];
    pub static ref API: Arc<Mutex<Api>> = Arc::new(Mutex::new(Api::default()));
}

#[derive(Clone, Routable, PartialEq)]
pub enum BaseRoute {
    #[at("/")]
    Home,
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

#[derive(Clone, PartialEq)]
pub struct OptionalValue<T>(Option<T>);

impl<T: std::fmt::Display> std::fmt::Display for OptionalValue<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(value) => write!(f, "{value}"),
            None => write!(f, ""),
        }
    }
}

impl std::str::FromStr for OptionalValue<String> {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Ok(Self(None));
        }
        Ok(Self(Some(s.to_string())))
    }
}

mod helpers;
mod pages;

fn main() {
    yew::Renderer::<Root>::new().render();
}

#[function_component]
fn Root() -> Html {
    html! {
        <BrowserRouter>
            <Switch<BaseRoute> render={switch} />
        </BrowserRouter>
    }
}

fn switch(routes: BaseRoute) -> Html {
    match routes {
        BaseRoute::Home => {
            html! {
                <pages::home::Home/>
            }
        }
        BaseRoute::BoardPage {
            board_discriminator,
        } => {
            html! {
                <pages::board_page::BoardPage board_discriminator={board_discriminator} />
            }
        }
        BaseRoute::ThreadPage {
            board_discriminator,
            thread_id,
        } => {
            html! {
                <pages::thread_page::ThreadPage board_discriminator={board_discriminator} thread_id={thread_id} />
            }
        }
        BaseRoute::NotFound => html! { <pages::not_found::NotFound/> },
    }
}

pub fn on_change_to_string(event: InputEvent) -> Option<String> {
    use wasm_bindgen::JsCast;
    match event.target() {
        Some(t) => {
            let t = t.dyn_into::<web_sys::HtmlInputElement>();
            match t {
                Ok(t) => Some(t.value()),
                Err(e) => {
                    let t = e.dyn_into::<web_sys::HtmlTextAreaElement>();
                    match t {
                        Ok(t) => Some(t.value()),
                        Err(e) => {
                            gloo::console::log!(format!("e: {e:?}"));
                            None
                        }
                    }
                }
            }
        }
        None => {
            gloo::console::log!("event target is none");
            None
        }
    }
}

pub fn get_name() -> Result<Option<String>> {
    Ok(gloo_storage::LocalStorage::get::<String>("name").map(|v| {
        if v.is_empty() {
            None
        } else {
            Some(v)
        }
    })?)
}

pub fn set_name(name: String) -> Result<()> {
    Ok(gloo_storage::LocalStorage::set("name", name)?)
}
