use gloo_storage::Storage;
use stylist::{css, style};
use yew::prelude::*;
use yew_router::prelude::*;
mod sidebar;

#[derive(Clone, Routable, PartialEq)]
enum PostAuthedRoute {
    #[at("/")]
    Home,
}

#[function_component]
pub fn Home() -> Html {
    // attempt to get user data from the API. If it fails, clear the token and redirect to the login page
    // if it succeeds, render the home page
    let valid = use_state(|| None);
    {
        let valid = valid.clone();
        use_effect_with_deps(
            move |_| {
                match gloo_storage::LocalStorage::get::<String>("token") {
                    Ok(t) => {
                        wasm_bindgen_futures::spawn_local(async move {
                            let fetch = gloo_net::http::Request::get(
                                "https://word.planetfifty.one/api/user/@me",
                            )
                            .header("authorization", &format!("Bearer {t}"))
                            .send()
                            .await;
                            match fetch {
                                Ok(f) => match f.json::<common::structs::User>().await {
                                    Ok(user) => {
                                        valid.set(Some(Some(user)));
                                    }
                                    Err(e) => {
                                        gloo::console::log!(format!("{e:?}"));
                                        gloo_storage::LocalStorage::delete("token");
                                        if let Some(w) = web_sys::window() {
                                            let _ = w.location().set_href("/login");
                                        }
                                    }
                                },
                                Err(_) => {
                                    valid.set(Some(None));
                                }
                            }
                        });
                    }
                    Err(_) => {
                        gloo_storage::LocalStorage::delete("token");
                        if let Some(w) = web_sys::window() {
                            let _ = w.location().set_href("/login");
                        }
                    }
                }
                || {}
            },
            (),
        );
    }
    match &*valid {
        Some(u) => {
            html! {
                <div class={css!("width: 100%; height: 100%; background-color: ${bg}; color: ${tx};", bg=crate::PALETTE.primary, tx=crate::PALETTE.text)}>
                    <div class={css!("width: auto; float: left; height: 100%; background-color: ${bg}; color: ${tx};", bg=crate::PALETTE.secondary, tx=crate::PALETTE.text)}>
                        <sidebar::Sidebar/>
                    </div>
                    <div class={css!("height: 100%; margin: 0px; background-color: ${bg}; color: ${tx};", bg=crate::PALETTE.primary, tx=crate::PALETTE.text)}>
                        <div class={css!("height: auto; width: auto; margin: 0px; padding: 0.2em; box-sizing: border-box;")}>{format!("{u:?}")}</div>
                    </div>
                </div>
            }
        }
        None => {
            html! {
                <div class="home">
                    <p>{"Loading..."}</p>
                </div>
            }
        }
    }
}
