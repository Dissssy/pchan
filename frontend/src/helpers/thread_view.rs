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
                        // gloo::console::log!("Using cache");
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

// impl PartialEq for MaybeExpandableThread {
//     fn eq(&self, other: &Self) -> bool {
//         match (self, other) {
//             (Self::Expandable(a), Self::Expandable(b)) => Arc::ptr_eq(a, b),
//             (Self::AlreadyExpanded(a), Self::AlreadyExpanded(b)) => a == b,
//             _ => false,
//         }
//     }
// }

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
//     pub fn expand_button(
//         &self,
//         board_discriminator: &str,
//         posts: UseStateSetter<Vec<SafePost>>,
//     ) -> Option<(Callback<MouseEvent>, String)> {
//         match self {
//             Self::Expandable(t) => {
//                 // bombent
//                 let expandtext;
//                 let loaded;
//                 if let Ok(t) = t.try_lock() {
//                     if !t.needs_expansion() {
//                         return None;
//                     }
//                     loaded = t.expanded.is_some();
//                     expandtext = if t.currently_expanded {
//                         format!("Collapse ({} posts)", t.minimized.post_count)
//                     } else {
//                         format!("Expand ({} posts)", t.minimized.post_count)
//                     };
//                 } else {
//                     return None;
//                 }
//                 let t = t.clone();
//                 let thread_id = format!("{}", self.thread_post().post_number);
//                 let board_discriminator = board_discriminator.to_string();
//                 Some((
//                     Callback::from(move |e: MouseEvent| {
//                         e.prevent_default();
//                         let t = t.clone();
//                         let thread_id = thread_id.clone();
//                         let board_discriminator = board_discriminator.clone();
//                         wasm_bindgen_futures::spawn_local(async move {
//                             gloo::console::log!("Expand button clicked");
//                             let mut set_to = None;
//                             if !loaded {
//                                 gloo::console::log!("Loading thread");
//                                 let threads = crate::API
//                                     .lock()
//                                     .await
//                                     .get_thread(&board_discriminator, &thread_id)
//                                     .await;
//                                 gloo::console::log!("Thread loaded");
//                                 match threads {
//                                     Ok(treds) => {
//                                         gloo::console::log!("Thread loaded successfully");
//                                         set_to = Some(treds);
//                                     }
//                                     Err(e) => {
//                                         gloo::console::log!(format!("Error loading thread: {}", e));
//                                     }
//                                 }
//                             }
//                             if let Ok(mut t) = t.lock() {
//                                 gloo::console::log!("Lock acquired");
//                                 if let Some(treds) = set_to {
//                                     gloo::console::log!("Setting expanded thread");
//                                     t.expanded = Some(treds);
//                                 }
//                                 gloo::console::log!("Toggling");
//                                 t.toggle();
//                             }
//                             gloo::console::log!("Done");
//                         });
//                     }),
//                     expandtext,
//                 ))
//             }
//             Self::AlreadyExpanded(_) => None,
//         }
//     }
//     pub fn thread_post_number(&self) -> i64 {
//         match self {
//             Self::Expandable(t) => match t.lock() {
//                 Ok(t) => t.thread_post().post_number,
//                 Err(_) => 0,
//             },
//             Self::AlreadyExpanded(t) => t.thread_post.post_number,
//         }
//     }
// }

// #[derive(Clone, PartialEq)]
// pub struct ExpandableThread {
//     minimized: ThreadWithLazyPosts,
//     expanded: Option<ThreadWithPosts>,
//     currently_expanded: bool,
// }

// impl ExpandableThread {
//     pub fn posts(&self) -> &Vec<SafePost> {
//         match (self.currently_expanded, &self.expanded) {
//             (true, Some(t)) => {
//                 gloo::console::log!("Returning expanded posts");
//                 &t.posts
//             }
//             _ => &self.minimized.posts,
//         }
//     }
//     pub fn thread_post(&self) -> &SafePost {
//         match (self.currently_expanded, &self.expanded) {
//             (true, Some(t)) => &t.thread_post,
//             _ => &self.minimized.thread_post,
//         }
//     }
//     pub fn toggle(&mut self) {
//         self.currently_expanded = !self.currently_expanded;
//     }
//     pub fn needs_expansion(&self) -> bool {
//         self.minimized.post_count > (self.minimized.posts.len() + 1) as i64
//     }
// }
