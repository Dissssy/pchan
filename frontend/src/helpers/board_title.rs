use yew::prelude::*;

use crate::pages::board_page::BoardWithThreads;

#[function_component]
pub fn BoardTitle(props: &TitleProps) -> Html {
    let board_info = use_state(|| None);

    {
        let board_info = board_info.clone();
        let props = props.clone();
        use_effect_with_deps(
            move |_| {
                wasm_bindgen_futures::spawn_local(async move {
                    let fetch = gloo_net::http::Request::get(&format!(
                        "/api/v1/board/{}",
                        props.board_discriminator
                    ))
                    .send()
                    .await;
                    match fetch {
                        Ok(f) => match f.json::<BoardWithThreads>().await {
                            Ok(boardses) => {
                                board_info.set(Some(boardses.name));
                            }
                            Err(e) => {
                                gloo::console::log!(format!("{e:?}"));
                            }
                        },
                        Err(e) => {
                            gloo::console::log!(format!("{e:?}"));
                        }
                    }
                });
                || {}
            },
            (),
        );
    }

    html! {
        <div class="board-title">
            <h1>
                {
                    match *board_info {
                        Some(ref b) => {
                            html! {
                                <>{format!("/{}/ - {}", props.board_discriminator, b)}</>
                            }
                        }
                        None => {
                            html! {
                                <>{""}</>
                            }
                        }
                    }
                }
            </h1>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct TitleProps {
    pub board_discriminator: String,
}
