use yew::prelude::*;

use crate::components::{BoardSelectBar, BoardTitle, PostBox};

#[function_component]
pub fn Header() -> Html {
    if *yew_hooks::use_local_storage::<bool>("verbose".to_owned()) == Some(true) {
        gloo::console::log!(format!("Refreshing Header"))
    }

    html! {
        <div class="board-header">
            <BoardSelectBar/>
            <h1>
                <BoardTitle/>
            </h1>
            <PostBox/>
        </div>
    }
}
