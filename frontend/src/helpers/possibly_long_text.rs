use yew::prelude::*;

use crate::helpers::rich_post_content::RichPostContent;

#[function_component]
pub fn TruncateText(props: &Props) -> Html {
    let ignore_max_length = props.ignore_max_length.unwrap_or(false);

    let expanded = use_state(|| false);

    let on_click = {
        let expanded = expanded.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            expanded.set(!*expanded);
        })
    };

    html! {
        <>
            {if props.text.len() > 250 && !ignore_max_length && !*expanded {
                html! {
                    <>
                        <RichPostContent text={props.text[..250].to_string()} invert={props.invert} this_thread_post_number={props.this_thread_post_number} load_posts={props.load_posts.clone()} board_discrim={props.board_discrim.clone()}/>
                        <span onclick={on_click} class="clickable">{format!("... (click to expand {} characters)", props.text.len() - 250)}</span>
                    </>
                }
            } else {
                html! {
                    <>
                        <RichPostContent text={props.text.clone()} invert={props.invert} this_thread_post_number={props.this_thread_post_number} load_posts={props.load_posts.clone()} board_discrim={props.board_discrim.clone()}/>
                    </>
                }
            }}
        </>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    pub text: String,
    pub ignore_max_length: Option<bool>,
    pub invert: bool,
    pub board_discrim: String,
    pub this_thread_post_number: i64,
    pub load_posts: Option<Callback<()>>,
}
