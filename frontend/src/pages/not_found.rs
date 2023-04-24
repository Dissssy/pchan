use yew::prelude::*;

use crate::helpers::board_list::BoardList;

#[function_component]
pub fn NotFound() -> Html {
    html! {
        <div class="center-vertical">
            <div class="center-horizontal">
                <div class="not-found">
                    <div class="not-found-text">
                        <h1>{"404 Not Found"}</h1>
                        <p>{"The page you requested could not be found."}</p>
                        <p>{"Maybe check out one of these fine boards instead?"}</p>
                    </div>
                    <BoardList />
                </div>
            </div>
        </div>
    }
}
