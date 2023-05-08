use yew::prelude::*;

use crate::components::{BannerAd, BoardSelectBar, BoardTitle, PostBox};

#[function_component]
pub fn Header() -> Html {
    html! {
        <div class="board-header">
            <BoardSelectBar/>
            <h1>
                <BoardTitle/>
            </h1>
            <BannerAd/>
            <PostBox/>
        </div>
    }
}
