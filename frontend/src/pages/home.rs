use crate::BaseRoute;
use yew::prelude::*;
use yew_router::{navigator, prelude::*};

#[function_component]
pub fn Home() -> Html {
    // make a request to /api/v1/board to get a list of boards

    let boards = use_state(|| None);
    {
        let boards = boards.clone();
        use_effect_with_deps(
            move |_| {
                wasm_bindgen_futures::spawn_local(async move {
                    let fetch = gloo_net::http::Request::get("/api/v1/board").send().await;
                    match fetch {
                        Ok(f) => match f.json::<Vec<common::structs::Board>>().await {
                            Ok(boardses) => {
                                boards.set(Some(Some(boardses)));
                            }
                            Err(e) => {
                                gloo::console::log!(format!("{e:?}"));
                            }
                        },
                        Err(_) => {
                            boards.set(Some(None));
                        }
                    }
                });
                || {}
            },
            (),
        );
    }

    match &*boards {
        Some(Some(b)) => {
            html! {
                <div class="home">
                    <div class="boards">
                        <h1>{"Boards"}</h1>
                        <table class="board-list" style="display: inline-block;">
                            <tr>
                                {for b.iter().map(|board| html! {
                                    <td class="board-link">
                                        <Link<BaseRoute> to={BaseRoute::BoardPage{board_discriminator: board.discriminator.clone()}}>
                                            {format!("/{}/ - {}", board.discriminator.clone(), board.name.clone())}
                                        </Link<BaseRoute>>
                                    </td>
                                })}
                            </tr>
                        </table>
                    </div>
                </div>
            }
        }
        Some(None) => {
            html! {
                <p>{"Error loading boards, more info in console"}</p>
            }
        }
        None => {
            html! {
                <p>{"Loading..."}</p>
            }
        }
    }
}
