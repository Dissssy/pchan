use gloo_storage::Storage;
use serde::Deserialize;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::BaseRoute;

#[derive(Clone, Routable, PartialEq)]
enum LoginRoute {
    #[at("/login")]
    Login,
    #[at("/login/callback")]
    Callback,
    #[not_found]
    #[at("/404")]
    NotFound,
}

#[function_component]
pub fn Login() -> Html {
    html! {
        <BrowserRouter>
            <Switch<LoginRoute> render={switch} />
        </BrowserRouter>
    }
}

fn switch(routes: LoginRoute) -> Html {
    match routes {
        LoginRoute::Login => {
            // prompt the user to login with discord oauth with a button to do so
            html! {
                <Navigate />
            }
        }
        LoginRoute::Callback => {
            html!(
                <CallbackHandler />
            )
        }
        LoginRoute::NotFound => html! { <h1>{"404"}</h1> },
    }
}

#[function_component]
pub fn CallbackHandler() -> Html {
    let code = yew_router::hooks::use_location()
        .and_then(|l| l.query::<CallbackResponse>().ok().map(|q| q.code));

    match code {
        Some(code) => {
            // save the code to local storage
            match gloo_storage::LocalStorage::set("token", code) {
                Ok(_) => {
                    // redirect to the home page
                    html! {
                        <Redirect<BaseRoute> to={BaseRoute::Home} />
                    }
                }
                Err(_) => {
                    // redirect to the login page
                    html! {
                        <Redirect<BaseRoute> to={BaseRoute::Login} />
                    }
                }
            }
        }
        None => {
            // redirect to the login page
            html! {
                <Redirect<BaseRoute> to={BaseRoute::Login} />
            }
        }
    }
}

#[derive(Deserialize)]
struct CallbackResponse {
    code: String,
}
