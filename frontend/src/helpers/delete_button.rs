use yew::prelude::*;

#[function_component]
pub fn DeleteButton(props: &Props) -> Html {
    let state = use_state(|| DeleteState::Untouched);

    let mvstate = state.clone();
    let mvprops = props.clone();
    let on_click = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        let state = mvstate.clone();
        match *state {
            DeleteState::ExclamationMark => {
                state.set(DeleteState::Pending);
                let post_number = mvprops.post_number;
                let board_discriminator = mvprops.board_discriminator.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let del = crate::API
                        .lock()
                        .await
                        .delete_post(&board_discriminator, &format!("{}", post_number))
                        .await;
                    match del {
                        Ok(_) => {
                            state.set(DeleteState::Complete(None));
                        }
                        Err(e) => {
                            state.set(DeleteState::Complete(Some(e.to_string())));
                        }
                    }
                });
            }
            DeleteState::Interrobang => {
                state.set(DeleteState::ExclamationMark);
            }
            DeleteState::QuestionMark => {
                state.set(DeleteState::Interrobang);
            }
            DeleteState::Untouched => {
                state.set(DeleteState::QuestionMark);
            }
            _ => {}
        }
    });

    let mvstate = state.clone();
    let on_mouseout = Callback::from(move |_: MouseEvent| match *mvstate {
        DeleteState::Pending => {}
        DeleteState::Complete(_) => {}
        _ => {
            mvstate.set(DeleteState::Untouched);
        }
    });

    html! {
        <div class="post-header-delete-button" onmouseout={on_mouseout}>
            {
                match *state {
                    DeleteState::Untouched => {
                        html! {
                            <a href="#" onclick={on_click}>
                                {"🗑️"}
                            </a>
                        }
                    }
                    DeleteState::QuestionMark => {
                        html! {
                            <a href="#" onclick={on_click}>
                                {"❓"}
                            </a>
                        }
                    }
                    DeleteState::Interrobang => {
                        html! {
                            <a href="#" onclick={on_click}>
                                {"⁉️"}
                            </a>
                        }
                    }
                    DeleteState::ExclamationMark => {
                        html! {
                            <a href="#" onclick={on_click}>
                                {"❗"}
                            </a>
                        }
                    }
                    DeleteState::Pending => {
                        html! {
                            <>{"⏳"}</>
                        }
                    }
                    DeleteState::Complete(None) => {
                        html! {
                            <>{"✅"}</>
                        }
                    }
                    DeleteState::Complete(Some(ref err)) => {
                        html! {
                            <span title={err.clone()}>
                                {"❌"}
                            </span>
                        }
                    }
                }
            }
        </div>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    pub post_number: i64,
    pub board_discriminator: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DeleteState {
    Untouched,
    QuestionMark,
    Interrobang,
    ExclamationMark,
    Pending,
    Complete(Option<String>),
}
