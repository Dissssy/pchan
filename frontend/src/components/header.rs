use yew::prelude::*;

use crate::components::{BoardSelectBar, BoardTitle, PostBox};

#[function_component]
pub fn Header() -> Html {
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
