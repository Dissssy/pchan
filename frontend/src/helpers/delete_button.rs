use yew::prelude::*;

#[function_component]
pub fn DeleteButton(props: &Props) -> Html {
    let state = use_state(|| DeleteState::Untouched);

    let mvstate = state.clone();
    let mvprops = props.clone();
    let callback = props.load_posts.clone();
    let on_click = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        let state = mvstate.clone();
        let (tstate, signal) = state.progress_with_trigger_signal();
        state.set(tstate);
        let callback = callback.clone();
        if signal {
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
                        if let Some(callback) = callback.clone() {
                            callback.emit(());
                        }
                        state.set(DeleteState::Complete(None));
                    }
                    Err(e) => {
                        state.set(DeleteState::Complete(Some(e.to_string())));
                    }
                }
            });
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
    pub load_posts: Option<Callback<()>>,
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

impl DeleteState {
    pub fn progress_with_trigger_signal(&self) -> (Self, bool) {
        let f = match *self {
            DeleteState::Untouched => DeleteState::QuestionMark,
            DeleteState::QuestionMark => DeleteState::Interrobang,
            DeleteState::Interrobang => DeleteState::ExclamationMark,
            DeleteState::ExclamationMark => DeleteState::Pending,
            DeleteState::Pending => DeleteState::Pending,
            DeleteState::Complete(ref e) => DeleteState::Complete(e.clone()),
        };
        let b = f == DeleteState::Pending;
        (f, b)
    }
}
