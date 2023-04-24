use yew::prelude::*;

use crate::helpers::board_list::BoardList;

#[function_component]
pub fn Home() -> Html {
    html! {
        <div class="center-vertical">
            <div class="center-horizontal">
                <div class="home">
                    <h1 class="board-title">{"PChan"}</h1>
                    <BoardList />
                </div>
            </div>
        </div>
    }
}
