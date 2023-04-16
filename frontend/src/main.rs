use stylist::{css, yew::Global};
use yew::prelude::*;
use yew_router::prelude::*;

#[derive(Clone, Routable, PartialEq)]
enum BaseRoute {
    #[at("/")]
    Home,
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
    match routes {
        BaseRoute::Home => {
            html! {
                <p>{"Hello world!"}</p>
            }
        }
        BaseRoute::NotFound => html! { <h1>{"404"}</h1> },
    }
}
