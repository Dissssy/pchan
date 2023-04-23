use yew::prelude::*;

#[function_component]
pub fn BoardTitle(props: &TitleProps) -> Html {
    let board_title = use_state(|| None);

    {
        let board_title = board_title.clone();
        let props = props.clone();
        use_effect_with_deps(
            move |_| {
                wasm_bindgen_futures::spawn_local(async move {
                    crate::API
                        .lock()
                        .await
                        .get_board_title(props.board_discriminator.clone(), board_title)
                        .await;
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
                    match *board_title {
                        Some(ref b) => {
                            html! {
                                <a href={format!("/{}/", props.board_discriminator)}>
                                    {format!("/{}/ - {}", props.board_discriminator, b)}
                                </a>
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
