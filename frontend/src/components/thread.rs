use std::sync::Arc;

use common::structs::{SafePost, ThreadWithLazyPosts, ThreadWithPosts};
use yew::{html::IntoPropValue, prelude::*};
use yew_router::prelude::*;

use crate::{
    api::Api,
    components::{ContextError, Post, Spinner, PostBox},
    ApiContext, BaseRoute, helpers::{CallbackEmitterContext, CallbackContext},
};

#[function_component]
pub fn Thread(props: &Props) -> Html {
    let api_ctx = use_context::<Option<ApiContext>>().flatten();
    let pending = use_state(|| false);
    let state = use_state(|| props.thread.clone_with_handle(pending.clone()));
    
    let add_text_callback = use_state(|| None);

    let set_add_text_callback = {
        let add_text_callback = add_text_callback.clone();
        CallbackEmitterContext {
            callback: Callback::from(move |callback: Callback<common::structs::Reply>| {
                add_text_callback.set(Some(CallbackContext { callback }));
            }),
        }
    };

    if let Some(board_discriminator) =
        use_route::<BaseRoute>().and_then(|r| r.board_discriminator())
    {
        if let Some(api) = api_ctx.and_then(|api_ctx| api_ctx.api.ok()) {
            let expand = {
                let state = state.clone();
                let board_discriminator = board_discriminator.clone();
                // let api = api.clone();
                Callback::from(move |e: MouseEvent| {
                    e.prevent_default();
                    if !state.pending() {
                        state.set_pending(true);
                        let state = state.clone();
                        let board_discriminator = board_discriminator.clone();
                        let api = api.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            let mut thread = (*state).clone();
                            if let ThreadState::Expandable(ref mut thread) = thread {
                                thread.expand(&board_discriminator, api).await;
                            }
                            if thread != *state {
                                state.set(thread);
                            }
                            state.set_pending(false);
                        });
                    }
                })
            };

            

            html! {
                <ContextProvider<CallbackEmitterContext> context={set_add_text_callback}>
                    <ContextProvider<Option<CallbackContext>> context={(*add_text_callback).clone()}>
                        <div class="thread-view">
                            <PostBox override_thread={props.thread.parent_post().thread_post_number.to_string()}/>
                            <Post post={state.parent_post().clone()} topic={ state.topic() } />
                            {
                                if let Some(text) = state.button_text() {
                                    html! {
                                        <div class="thread-view-expand-button">
                                            <a href={format!("/{}/{}", board_discriminator, state.parent_post().post_number)} onclick={expand} disabled={state.pending()}>{ text }</a>
                                            {
                                                if state.pending() {
                                                    html! {
                                                        <Spinner />
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }
                            }
                            {
                                for state.posts().iter().map(|post| {
                                    html! {
                                        <Post post={post.clone()} />
                                    }
                                })
                            }
                        </div>
                    </ContextProvider<Option<CallbackContext>>>
                </ContextProvider<CallbackEmitterContext>>
            }
        } else {
            html! {
                <ContextError cause={"ApiContext"} source={"Thread"}/>
            }
        }
    } else {
        html! {
            <ContextError cause={"BaseRoute (board_discriminator)"} source={"Thread"}/>
        }
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub thread: ThreadState,
}

#[derive(Clone, PartialEq)]
pub enum ThreadState {
    Full(Box<ThreadWithPosts>),
    Expandable(Box<ExpandableThread>),
}

impl ThreadState {
    pub fn parent_post(&self) -> &SafePost {
        match self {
            ThreadState::Expandable(thread) => &thread.thread.thread_post,
            ThreadState::Full(thread) => &thread.thread_post,
        }
    }

    pub fn posts(&self) -> &Vec<SafePost> {
        match self {
            ThreadState::Expandable(thread) => thread.posts(),
            ThreadState::Full(thread) => &thread.posts,
        }
    }

    pub fn post_count(&self) -> usize {
        match self {
            ThreadState::Expandable(thread) => thread.thread.post_count as usize,
            ThreadState::Full(thread) => thread.post_count as usize,
        }
    }

    pub fn pending(&self) -> bool {
        match self {
            ThreadState::Expandable(thread) => match thread.pending {
                Some(ref pending) => **pending,
                None => false,
            },
            ThreadState::Full(_) => false,
        }
    }

    pub fn set_pending(&self, pending: bool) {
        if let ThreadState::Expandable(thread) = self {
            if let Some(ref pendinghandle) = thread.pending {
                pendinghandle.set(pending)
            }
        }
    }

    pub fn clone_with_handle(&self, handle: UseStateHandle<bool>) -> Self {
        match self {
            ThreadState::Expandable(thread) => {
                ThreadState::Expandable(Box::new(ExpandableThread {
                    thread: thread.thread.clone(),
                    full_thread: thread.full_thread.clone(),
                    expanded: thread.expanded,
                    pending: Some(handle),
                }))
            }
            ThreadState::Full(thread) => ThreadState::Full(thread.clone()),
        }
    }

    pub fn topic(&self) -> String {
        match self {
            ThreadState::Expandable(thread) => thread.thread.topic.clone(),
            ThreadState::Full(thread) => thread.topic.clone(),
        }
    }

    pub fn show_post_box(&self) -> bool {
        match self {
            ThreadState::Expandable(_) => true,
            ThreadState::Full(_) => false,
        }
    }

    // pub fn show_button(&self) -> bool {
    //     match self {
    //         ThreadState::Expandable(thread) => thread.thread.post_count != thread.thread.posts.len() as i64,
    //         ThreadState::Full(_) => false,
    //     }
    // }

    pub fn button_text(&self) -> Option<String> {
        // Show ({hidden_post_count} posts)
        // Hide ({hidden_post_count} posts)

        match self {
            ThreadState::Expandable(thread) => {
                if thread.thread.post_count != thread.thread.posts.len() as i64 {
                    let hidden_post_count =
                        thread.thread.post_count - thread.thread.posts.len() as i64;
                    let text = if thread.expanded {
                        format!("Hide {} posts", hidden_post_count)
                    } else {
                        format!("Show {} posts", hidden_post_count)
                    };
                    Some(text)
                } else {
                    None
                }
            }
            ThreadState::Full(_) => None,
        }
    }
}

impl IntoPropValue<ThreadState> for ThreadWithLazyPosts {
    fn into_prop_value(self) -> ThreadState {
        ThreadState::Expandable(Box::new(ExpandableThread {
            thread: self,
            full_thread: None,
            expanded: false,
            pending: None,
        }))
    }
}

impl IntoPropValue<ThreadState> for ThreadWithPosts {
    fn into_prop_value(self) -> ThreadState {
        ThreadState::Full(Box::new(self))
    }
}

#[derive(Clone, PartialEq)]
pub struct ExpandableThread {
    thread: ThreadWithLazyPosts,
    full_thread: Option<ThreadWithPosts>,
    expanded: bool,
    pending: Option<UseStateHandle<bool>>,
}

impl ExpandableThread {
    pub async fn expand(&mut self, board: &str, api: Arc<Api>) {
        self.expanded = !self.expanded;
        if self.expanded && self.full_thread.is_none() {
            match api
                .get_thread(board, &self.thread.thread_post.post_number.to_string())
                .await
            {
                Ok(t) => self.full_thread = Some(t),
                Err(e) => {
                    log::error!("Error getting thread: {:?}", e);
                    self.expanded = false;
                }
            }
        }
    }
    pub fn posts(&self) -> &Vec<SafePost> {
        match (self.expanded, &self.full_thread) {
            (true, Some(thread)) => &thread.posts,
            _ => &self.thread.posts,
        }
    }
}
