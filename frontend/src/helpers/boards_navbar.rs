use yew::prelude::*;

use crate::helpers::board_link::BoardLink;

#[function_component]
pub fn NavBar(props: &Props) -> Html {
    let boards = use_state(|| None);

    {
        let boards = boards.clone();
        use_effect_with_deps(
            move |_| {
                wasm_bindgen_futures::spawn_local(async move {
                    crate::API.lock().await.get_boards(boards).await;
                });
                || {}
            },
            (),
        );
    }

    html! {
        <div class="navbar">
            {
                match *boards {
                    Some(ref b) => {
                        html! {
                            {
                                for b.iter().map(|board| html! {
                                    <BoardLink board={board.clone()} board_discriminator={props.board_discriminator.clone()} />
                                })
                            }
                        }
                    },
                    None => {
                        html! {
                        }
                    }
                }
            }
        </div>
    }
}

#[derive(Clone, Properties, PartialEq)]
pub struct Props {
    pub board_discriminator: String,
}
