use yew::prelude::*;

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
                                    <span class={ if board.discriminator == props.board_discriminator { "current-navbar-link" } else { "navbar-link" }} title={board.name.clone()}>
                                        <a href={format!("/{}/", board.discriminator.clone())}>
                                            {format!("/{}/", board.discriminator.clone())}
                                        </a>
                                    </span>
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
