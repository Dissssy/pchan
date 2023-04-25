use common::structs::Reply;
use yew::prelude::*;

use crate::helpers::{lazy_post::LazyPost, startswith_class::StartsWithClass};

#[function_component]
pub fn RichPostContent(props: &Props) -> Html {
    let mut last_empty = false;
    let mut first = true;
    html! {
        <>
            {
                for props.text.lines().map(|l| {
                    if l.is_empty() && !last_empty {
                        last_empty = true;
                        html! {
                            <>
                                <br />
                            </>
                        }
                    } else if l.is_empty() {
                        last_empty = true;
                        html! {}
                    } else {
                        last_empty = false;
                        html! {
                            <>
                                {
                                    if !first {
                                        html! {
                                            <br />
                                        }
                                    } else {
                                        first = false;
                                        html! {}
                                    }
                                }
                                {
                                    if let Ok(r) = Reply::from_str(l, &props.board_discrim) {
                                        html! {
                                            <LazyPost reply={r} this_board={props.board_discrim.clone()} invert={props.invert} this_thread_post_number={props.this_thread_post_number} load_posts={props.load_posts.clone()} />
                                        }
                                    } else {
                                        html! {
                                            <StartsWithClass text={l.to_owned()} map={crate::CLASSMAP.clone()} />
                                        }
                                    }
                                }
                            </>
                        }
                    }
                })
            }
        </>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    pub text: String,
    pub invert: bool,
    pub board_discrim: String,
    pub this_thread_post_number: i64,
    pub load_posts: Option<Callback<()>>,
}
