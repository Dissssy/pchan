use common::structs::{SafePost, ThreadWithLazyPosts, ThreadWithPosts};
use yew::prelude::*;

use crate::helpers::post_container::PostView;

#[function_component]
pub fn ThreadView(props: &Props) -> Html {
    let expanded = use_state(|| false);

    let posts = use_state(|| props.thread.posts().clone());
    if &*posts != props.thread.posts() && !*expanded {
        posts.set(props.thread.posts().clone());
    }

    let cache = use_state(|| None::<Vec<SafePost>>);

    let texpanded = expanded.clone();
    let tposts = posts.clone();
    let tthread = props.thread.clone();
    let tboard_discriminator = props.board_discriminator.clone();
    let callback = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        if let MaybeExpandableThread::Expandable(ref t) = tthread {
            match *texpanded {
                true => {
                    tposts.set(tthread.posts().clone());
                    texpanded.set(false);
                }
                false => match *cache {
                    Some(ref c) => {
                        tposts.set(c.clone());
                        texpanded.set(true);
                    }
                    None => {
                        let cache = cache.clone();
                        let posts = tposts.clone();
                        let expanded = texpanded.clone();
                        let board_discriminator = tboard_discriminator.clone();
                        let thread_id = t.thread_post.post_number;
                        wasm_bindgen_futures::spawn_local(async move {
                            let newposts = crate::API
                                .lock()
                                .await
                                .get_thread(&board_discriminator, &format!("{}", thread_id))
                                .await;
                            match newposts {
                                Ok(newposts) => {
                                    let posterino = newposts.posts.clone();
                                    posts.set(newposts.posts);
                                    cache.set(Some(posterino));
                                    expanded.set(true);
                                }
                                Err(e) => {
                                    gloo::console::log!(format!("{e:?}"));
                                }
                            }
                        });
                    }
                },
            }
        }
    });

    html! {
        <div class="threadposts-list">
            <div class="threadposts-post">
                <PostView post={props.thread.thread_post().clone()} board_discrim={props.board_discriminator.clone()} topic={props.thread.topic().clone()} add_to_content={props.add_to_content.clone()} this_thread_post_number={props.thread.thread_post().post_number} />
            </div>
            {
                match props.thread {
                    MaybeExpandableThread::Expandable(ref t) => {
                        let post_diff = t.post_count as usize - t.posts.len();
                        if post_diff > 1 {
                            html! {
                                <a href={format!("/{}/thread/{}", props.board_discriminator, t.thread_post.post_number)} onclick={callback}>
                                    <div class="expand-link">
                                        {
                                            if *expanded { "Collapse".to_owned() } else { format!("Expand ({} posts hidden)", post_diff) }
                                        }
                                    </div>
                                </a>
                            }
                        } else {
                            html! {}
                        }
                    }
                    _ => html! {}
                }
            }
            <div class="threadposts-replies">
                {
                    for posts.iter().map(|p| {
                        html! {
                            <div class="threadposts-post">
                                <PostView post={p.clone()} board_discrim={props.board_discriminator.clone()} add_to_content={props.add_to_content.clone()} this_thread_post_number={props.thread.thread_post().post_number} />
                            </div>
                        }
                    })
                }
            </div>
        </div>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    pub thread: MaybeExpandableThread,
    pub rerender: bool,
    pub add_to_content: Option<UseStateHandle<String>>,
    pub board_discriminator: String,
}

#[derive(Clone, PartialEq)]
pub enum MaybeExpandableThread {
    Expandable(ThreadWithLazyPosts),
    AlreadyExpanded(ThreadWithPosts),
}

impl From<ThreadWithLazyPosts> for MaybeExpandableThread {
    fn from(t: ThreadWithLazyPosts) -> Self {
        Self::Expandable(t)
    }
}

impl From<ThreadWithPosts> for MaybeExpandableThread {
    fn from(t: ThreadWithPosts) -> Self {
        Self::AlreadyExpanded(t)
    }
}

impl MaybeExpandableThread {
    pub fn posts(&self) -> &Vec<SafePost> {
        match self {
            Self::Expandable(t) => &t.posts,
            Self::AlreadyExpanded(t) => &t.posts,
        }
    }
    pub fn thread_post(&self) -> &SafePost {
        match self {
            Self::Expandable(t) => &t.thread_post,
            Self::AlreadyExpanded(t) => &t.thread_post,
        }
    }
    pub fn topic(&self) -> &String {
        match self {
            Self::Expandable(t) => &t.topic,
            Self::AlreadyExpanded(t) => &t.topic,
        }
    }
}
