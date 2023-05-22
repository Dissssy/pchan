use yew::prelude::*;
use yew_router::prelude::use_route;

use crate::{
    components::{BannerAd, BoardSelectBar, BoardTitle, PostBox},
    BaseRoute,
};

#[function_component]
pub fn Header() -> Html {
    let post_box = use_route::<BaseRoute>().map_or(false, |r| r.thread_id().is_none());

    html! {
        <div class="board-header">
            <BoardSelectBar/>
            <h1>
                <BoardTitle/>
            </h1>
            <BannerAd/>
            {
                if post_box {
                    html! {
                        <PostBox/>
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}
