use stylist::{css, yew::Global};
use yew::prelude::*;
use yew_router::prelude::*;

#[derive(Clone, Routable, PartialEq)]
pub enum BaseRoute {
    #[at("/")]
    Home,
    // /{board_discriminator}
    #[at("/:board_discriminator/")]
    BoardPage { board_discriminator: String },
    // /{board_discriminator}/{thread_id}
    #[at("/:board_discriminator/:thread_id")]
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

mod pages;

fn main() {
    yew::Renderer::<Root>::new().render();
}

#[function_component]
fn Root() -> Html {
    // This is the root app component. this will determind if the user is logged in via the token cookie
    // if the user is logged in, then the user will be permitted to access the app, otherwise they will be redirected to the login page

    html! {
        <BrowserRouter>
            <Switch<BaseRoute> render={switch} /> // <- must be child of <BrowserRouter>
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
        BaseRoute::NotFound => html! { <h1>{"404"}</h1> },
    }
}

pub fn on_change_to_string(event: Event) -> Option<String> {
    use wasm_bindgen::JsCast;
    match event.target() {
        Some(t) => {
            gloo::console::log!(format!("t: {t:?}"));
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

// pub fn get_cookie(c: &str) -> Option<String> {
//     use wasm_bindgen::JsCast;
//     // let window = web_sys::window().map(|w| {
//     //     w.document().map(|d| {
//     //         d.dyn_into::<web_sys::HtmlDocument>()
//     //             .map(|d| d.cookie().map(|coke| wasm_cookies::cookies::get(&coke, c)))
//     //     })
//     // });
//     // match window {
//     //     Some(Some(Ok(Ok(Some(Ok(cookie)))))) => Some(cookie),
//     //     _ => None,
//     // }
//     match web_sys::window() {
//         None => {
//             gloo::console::log!("window is none");
//             None
//         }
//         Some(window) => match window.document() {
//             None => {
//                 gloo::console::log!("document is none");
//                 None
//             }
//             Some(document) => match document.dyn_into::<web_sys::HtmlDocument>() {
//                 Err(e) => {
//                     gloo::console::log!(format!("Error: {e:?}"));
//                     None
//                 }
//                 Ok(document) => match document.cookie() {
//                     Err(e) => {
//                         gloo::console::log!(format!("Error: {e:?}"));
//                         None
//                     }
//                     Ok(cookie) => match wasm_cookies::cookies::get(&cookie, c) {
//                         None => {
//                             gloo::console::log!(format!(
//                                 "cookie is none, cookie string: {cookie:?}"
//                             ));
//                             None
//                         }
//                         Some(Err(e)) => {
//                             gloo::console::log!(format!("Error: {e:?}"));
//                             None
//                         }
//                         Some(Ok(cookie)) => Some(cookie),
//                     },
//                 },
//             },
//         },
//     }
// }
