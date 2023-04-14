use gloo_storage::Storage;
use log::error;
use stylist::{css, yew::Global};
use yew::prelude::*;
use yew_router::prelude::*;
mod pages;
mod utils;

pub const PALETTE: Palette = Palette {
    primary: "#F6EBCB",
    secondary: "#895B1E",
    accent: "#35524A",
    text: "#222200",
    notice: "#32DE8A",
};

#[derive(Clone, Routable, PartialEq)]
enum BaseRoute {
    #[at("/")]
    Home,
    #[at("/login")]
    Login,
    #[at("/storage/set/:key/:value/:callback")]
    SetLocalStorage {
        key: String,
        value: String,
        callback: String,
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
    // This is the root app component. this will determind if the user is logged in via the token cookie
    // if the user is logged in, then the user will be permitted to access the app, otherwise they will be redirected to the login page

    html! {
        <BrowserRouter>
            <Global css={css!("margin: 0px; padding: 0.4em; padding-top: 0.2em; box-sizing: border-box;")} />
            <Switch<BaseRoute> render={switch} /> // <- must be child of <BrowserRouter>
        </BrowserRouter>
    }
}

fn switch(routes: BaseRoute) -> Html {
    let token = gloo_storage::LocalStorage::get::<String>("token");

    match routes {
        BaseRoute::Home => {
            if token.is_ok() {
                html! {
                    <pages::home::Home/>
                }
            } else {
                html! {
                    <Redirect<BaseRoute> to={BaseRoute::Login} />
                }
            }
        }
        BaseRoute::Login => {
            if let Some(w) = web_sys::window() {
                let _ = w.location().set_href(env!("OAUTH_URL"));
            }
            html! {
                <div>
                    <p>{"Redirecting..."}</p>
                    <a href={env!("OAUTH_URL")}>{"If you are not redirected automatically, click here."}</a>
                </div>
            }
        }
        BaseRoute::SetLocalStorage {
            key,
            value,
            callback,
        } => match gloo_storage::LocalStorage::set(key, value) {
            Ok(_) => {
                let callback = if callback == "root" {
                    "/".to_string()
                } else {
                    callback
                };
                let e = web_sys::window().map(|w| {
                    w.location()
                        .set_href(&callback)
                        .map_err(|e| format!("Error redirecting: {e:?}"))
                });
                html! {
                    <div>
                        <p>{"Redirecting..."}</p>
                        {
                            if let Some(Err(e)) = e {
                                error!("{e:?}");
                                html! {
                                    <p style="color:red;">{e}</p>
                                }
                            } else {
                                html! {}
                            }
                        }
                        <a href={callback}>{"If you are not redirected automatically, click here."}</a>
                    </div>
                }
            }
            Err(e) => html! {
                <p>{format!("Error: {e:?}")}</p>
            },
        },
        BaseRoute::NotFound => html! { <h1>{"404"}</h1> },
    }
}

pub struct Palette<'a> {
    primary: &'a str,
    secondary: &'a str,
    accent: &'a str,
    text: &'a str,
    notice: &'a str,
}
