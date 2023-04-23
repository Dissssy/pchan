use yew::prelude::*;

#[function_component]
pub fn Home() -> Html {
    // make a request to /api/v1/board to get a list of boards

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

    match &*boards {
        Some(b) => {
            html! {
                <div class="home">
                    <div class="boards">
                        <h1 class="board-title">{"PChan"}</h1>
                        <table class="board-list">
                            <tr>
                                {for b.iter().map(|board| html! {
                                    <td class="board-link">
                                        <a href={format!("/{}/", board.discriminator)}>
                                            {board.name.clone()}
                                        </a>
                                    </td>
                                })}
                            </tr>
                        </table>
                    </div>
                </div>
            }
        }
        None => {
            html! {
                <p>{"Loading..."}</p>
            }
        }
    }
}
