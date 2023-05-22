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
    pub content: AttrValue,
    pub board: AttrValue,
    pub thread_post_number: AttrValue,
    pub invert: bool,
}

pub struct RichTextContent {
    pub class: AttrValue,
    pub string: AttrValue,
}

lazy_static::lazy_static! {
    static ref RICHFILTERS: Vec<fn(&str) -> Option<RichTextContent>> = vec![
        |s| {
            s.starts_with('>').then(|| RichTextContent {
                class: AttrValue::from("bluetext"),
                string: AttrValue::from(s.to_owned()),
            })
        },
        |s| {
            s.starts_with('<').then(|| RichTextContent {
                class: AttrValue::from("peetext"),
                string: AttrValue::from(s.to_owned()),
            })
        },
        |s| {
            return s.strip_prefix(r#"./*"#).and_then(|s| return s.strip_suffix(r#"*\."#)).map(|s| RichTextContent {
                class: AttrValue::from("gibberish"),
                string: AttrValue::from(s.to_owned()),
            })
        },
    ];
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
        RICHFILTERS
            .iter()
            .map(|f| f(&props.line))
            .find(|s| s.is_some())
            .flatten(),
    ) {
        (Ok(reply), _) => {
            html! {
                <Reply reply={reply} thread_post_number={props.thread_post_number.clone()} invert={props.invert}/>
            }
        }
        (_, Some(richtextinfo)) => {
            html! {
                <span class={richtextinfo.class.to_string()}><SpoilableText content={richtextinfo.string} /></span>
            }
        }
        _ => {
            html! {
                <span><SpoilableText content={props.line.clone()} /></span>
            }
        }
    }
}

#[derive(Clone, PartialEq, Properties)]
struct RichLineProps {
    pub line: AttrValue,
    pub board: AttrValue,
    pub thread_post_number: AttrValue,
    pub invert: bool,
}

#[function_component]
pub fn SpoilableText(props: &SpoilableProps) -> Html {
    // wrap any text enclosed in | in a <span class="spoiler"> tag, ending at the next | with a </span>. if there is no next | do not insert a <span class="spoiler"> tag
    let mut splits = props.content.split("||");

    let first = splits.next().unwrap_or_default();
    let following = splits
        .map(|s| AttrValue::from(s.to_owned()))
        .collect::<Vec<AttrValue>>();

    html! {
        <>
            {first}
            {
                // iterate over the splits in pairs
                following.chunks(2).map(|s| {

                    if let Some(next) = s.get(1) {
                        html! {
                            <>
                                <SpoiledText content={s.get(0).unwrap_or(&AttrValue::from("this shouldn't happen")).clone()} />
                                {next}
                            </>
                        }
                    } else {
                        html! {
                            <>{format!("||{}", s.get(0).unwrap_or(&AttrValue::from("this shouldn't happen")))}</>
                        }
                    }

                }).collect::<Html>()
            }
        </>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct SpoilableProps {
    pub content: AttrValue,
}

#[function_component]
pub fn SpoiledText(props: &SpoilableProps) -> Html {
    let shown = use_state(|| false);

    let on_click = {
        let shown = shown.clone();
        Callback::from(move |_| {
            shown.set(!*shown);
        })
    };

    if props.content.is_empty() {
        html! {}
    } else if *shown {
        html! {
            <span class="spoiler-shown" onclick={on_click}>{props.content.clone()}</span>
        }
    } else {
        html! {
            <span class="spoiler" onclick={on_click}>{props.content.clone()}</span>
        }
    }
}
