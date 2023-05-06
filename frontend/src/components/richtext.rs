use yew::prelude::*;

use crate::components::Reply;

#[function_component]
pub fn RichText(props: &Props) -> Html {
    let p = props.content.clone();
    let mut lines = p.lines().map(|s| s.to_owned()).rev();
    let last = lines.next().unwrap_or_default();

    let thread_post_number = props.thread_post_number.clone();
    let invert = props.invert;

    html! {
        <div class="post-rich-content">
            {
                for lines.rev().map(|line| {
                    html! {
                        <>
                            <RichLine board={props.board.clone()} line={line.clone()} thread_post_number={thread_post_number.clone()} invert={invert} />
                            <br />
                        </>
                    }
                })
            }
            <RichLine board={props.board.clone()} line={last.clone()} thread_post_number={thread_post_number.clone()} invert={invert} />
        </div>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    pub content: String,
    pub board: String,
    pub thread_post_number: String,
    pub invert: bool,
}

#[function_component]
fn RichLine(props: &RichLineProps) -> Html {
    // basically just attempt to parse out the different fancy line states :D
    // possibilities are:
    // Reply::from_str()
    // >{} (is bluetext)
    // <{} (is peetext)
    // otherwise plaintext

    match (
        common::structs::Reply::from_str(&props.line, &props.board, &props.thread_post_number),
        props.line.starts_with('>'),
        props.line.starts_with('<'),
    ) {
        (Ok(reply), _, _) => {
            html! {
                <Reply reply={reply} thread_post_number={props.thread_post_number.clone()} invert={props.invert}/>
            }
        }
        (_, true, _) => {
            html! {
                <span class="bluetext">{props.line.clone()}</span>
            }
        }
        (_, _, true) => {
            html! {
                <span class="peetext">{props.line.clone()}</span>
            }
        }
        _ => {
            html! {
                <span>{props.line.clone()}</span>
            }
        }
    }
}

#[derive(Clone, PartialEq, Properties)]
struct RichLineProps {
    pub line: String,
    pub board: String,
    pub thread_post_number: String,
    pub invert: bool,
}
