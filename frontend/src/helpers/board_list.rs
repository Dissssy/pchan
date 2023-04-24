use yew::prelude::*;

#[function_component]
pub fn BoardList() -> Html {
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
                    <div class="boards">
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
            }
        }
        None => {
            html! {
                <p>{"Loading..."}</p>
            }
        }
    }
}
