use serde::Deserialize;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::pages::post_box::PostBox;

#[function_component]
pub fn ThreadPage(props: &Props) -> Html {
    // get reply value from query string
    let reply = match use_location().map(|l| l.query::<Reply>()) {
        Some(Ok(query)) => query.reply,
        Some(Err(e)) => {
            gloo::console::log!(format!("{e:?}"));
            None
        }
        None => None,
    };

    html! {
        <div>
            <PostBox board_discriminator={props.board_discriminator.clone()} thread_id={props.thread_id.clone()} starter_text={reply} />
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub board_discriminator: String,
    pub thread_id: String,
}

#[derive(Deserialize, Clone, PartialEq)]
pub struct Reply {
    reply: Option<String>,
}
