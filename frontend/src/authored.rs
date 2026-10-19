//! What this browser remembers about which posts the member wrote.
//!
//! The server will say, for a batch of posts, which are the asker's own: it is the same check
//! deleting a post makes. Asking costs a request, so answers are kept here. That lets a post
//! be drawn differently when it is yours, and lets a notification say a post replies to one
//! of yours, without the server ever recording who answers whom.
//!
//! The memory is tied to the member through `MeInfo::cache_key`. A different member signing in
//! on the same browser, or a new posting identity, changes the key, and everything remembered
//! is dropped, so one person's authorship is never shown to the next. It is also dropped on
//! log out.
//!
//! Posts the member makes are recorded the moment they are made. The rest are asked about in
//! batches as they come on screen, slowly: the server allows one request a second, and the
//! answers are kept, so each post is only ever asked about once.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};

use common::structs::PostRef;
use serde::{Deserialize, Serialize};
use yew::Callback;

use crate::api::Api;

const STORAGE_KEY: &str = "authored_posts";
/// Entries kept. Past this the oldest "not mine" answers go first.
const MAX_ENTRIES: usize = 5_000;
/// Matches the most the server accepts in one request.
const BATCH: usize = 200;
/// Waiting briefly lets everything that came on screen together go in one request.
const FLUSH_DELAY_MS: u32 = 300;
/// The server allows one request a second per member.
const SPACING_MS: u32 = 1_100;
const RETRIES: u32 = 3;

fn key(board: &str, number: i64) -> String {
    format!("{board}/{number}")
}

/// How it is saved: two lists, which is smaller than a map and keeps the order.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
struct Stored {
    owner: String,
    mine: Vec<String>,
    not_mine: Vec<String>,
}

/// The answers themselves, without anything that touches the browser.
#[derive(Debug, Default)]
struct Known {
    owner: String,
    answers: HashMap<String, bool>,
    /// Oldest first.
    order: VecDeque<String>,
}

impl Known {
    fn from_stored(stored: Stored) -> Self {
        let mut known = Self {
            owner: stored.owner,
            ..Default::default()
        };
        for k in stored.not_mine {
            known.set(k, false);
        }
        for k in stored.mine {
            known.set(k, true);
        }
        known
    }

    fn to_stored(&self) -> Stored {
        let mut stored = Stored {
            owner: self.owner.clone(),
            ..Default::default()
        };
        for k in &self.order {
            match self.answers.get(k) {
                Some(true) => stored.mine.push(k.clone()),
                Some(false) => stored.not_mine.push(k.clone()),
                None => {}
            }
        }
        stored
    }

    fn get(&self, k: &str) -> Option<bool> {
        self.answers.get(k).copied()
    }

    fn set(&mut self, k: String, mine: bool) {
        if self.answers.insert(k.clone(), mine).is_none() {
            self.order.push_back(k);
        }
        while self.answers.len() > MAX_ENTRIES {
            // A "yes" cost the member something to learn and cannot be re-derived for free in
            // the same way; a "no" is the cheapest thing to ask again.
            let victim = self
                .order
                .iter()
                .position(|k| self.answers.get(k) == Some(&false))
                .unwrap_or(0);
            if let Some(k) = self.order.remove(victim) {
                self.answers.remove(&k);
            }
        }
    }

    /// Tie the memory to `owner`, dropping it if it belonged to someone else. True if it
    /// changed.
    fn bind(&mut self, owner: &str) -> bool {
        if self.owner == owner {
            return false;
        }
        *self = Self {
            owner: owner.to_owned(),
            ..Default::default()
        };
        true
    }
}

#[derive(Default)]
struct State {
    known: Option<Known>,
    /// Waiting to be asked about.
    pending: Vec<PostRef>,
    /// Pending or in a request that has not come back: no need to want these again.
    wanted: HashSet<String>,
    flushing: bool,
    notify: Option<Callback<()>>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with_known<R>(f: impl FnOnce(&mut Known, &mut State) -> R) -> R {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let mut known = state.known.take().unwrap_or_else(load);
        let out = f(&mut known, &mut state);
        state.known = Some(known);
        out
    })
}

fn load() -> Known {
    use gloo::storage::{LocalStorage, Storage};
    LocalStorage::get::<Stored>(STORAGE_KEY)
        .map(Known::from_stored)
        .unwrap_or_default()
}

fn save(known: &Known) {
    use gloo::storage::{LocalStorage, Storage};
    if let Err(e) = LocalStorage::set(STORAGE_KEY, known.to_stored()) {
        gloo::console::warn!(format!("could not save the authorship cache: {e}"));
    }
}

fn changed() {
    let notify = STATE.with(|s| s.borrow().notify.clone());
    if let Some(notify) = notify {
        notify.emit(());
    }
}

/// Whether the member wrote this post, if this browser knows.
pub fn lookup(board: &str, number: i64) -> Option<bool> {
    with_known(|known, _| known.get(&key(board, number)))
}

/// The member just made this post.
pub fn record_mine(board: &str, number: i64) {
    with_known(|known, _| {
        known.set(key(board, number), true);
        save(known);
    });
    changed();
}

/// Tie the memory to this member. Call with `MeInfo::cache_key` once it is known.
pub fn bind_owner(owner: &str) {
    let reset = with_known(|known, state| {
        let reset = known.bind(owner);
        if reset {
            save(known);
            state.pending.clear();
            state.wanted.clear();
        }
        reset
    });
    if reset {
        changed();
    }
}

/// Forget everything, e.g. on log out.
pub fn clear() {
    with_known(|known, state| {
        *known = Known::default();
        save(known);
        state.pending.clear();
        state.wanted.clear();
    });
    changed();
}

/// Be told when something was learned, so what is drawn can be redrawn.
pub fn set_notifier(notify: Callback<()>) {
    STATE.with(|s| s.borrow_mut().notify = Some(notify));
}

/// Ask about any of these posts not already known. Cheap to call for everything on screen.
pub fn want(api: Arc<Api>, posts: impl IntoIterator<Item = (String, i64)>) {
    let start = with_known(|known, state| {
        for (board, number) in posts {
            let k = key(&board, number);
            if known.get(&k).is_none() && state.wanted.insert(k) {
                state.pending.push(PostRef {
                    board_discriminator: board,
                    post_number: number,
                });
            }
        }
        let start = !state.pending.is_empty() && !state.flushing;
        if start {
            state.flushing = true;
        }
        start
    });
    if start {
        wasm_bindgen_futures::spawn_local(flush(api));
    }
}

fn take_batch() -> Vec<PostRef> {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let n = s.pending.len().min(BATCH);
        s.pending.drain(..n).collect()
    })
}

async fn flush(api: Arc<Api>) {
    use gloo_timers::future::TimeoutFuture;

    TimeoutFuture::new(FLUSH_DELAY_MS).await;
    loop {
        let batch = take_batch();
        if batch.is_empty() {
            break;
        }

        let mut attempt = 0;
        let answer = loop {
            match api.authored(batch.clone()).await {
                Ok(mine) => break Some(mine),
                // Most likely the one-a-second limit; wait it out.
                Err(_) if attempt < RETRIES => {
                    attempt += 1;
                    TimeoutFuture::new(SPACING_MS * 2).await;
                }
                Err(_) => break None,
            }
        };

        with_known(|known, state| {
            for post in &batch {
                let k = key(&post.board_discriminator, post.post_number);
                state.wanted.remove(&k);
                if let Some(mine) = &answer {
                    known.set(k, mine.contains(post));
                }
                // On failure it is simply asked about again the next time it is wanted.
            }
            save(known);
        });
        changed();
        TimeoutFuture::new(SPACING_MS).await;
    }
    STATE.with(|s| s.borrow_mut().flushing = false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_and_forgets_by_owner() {
        let mut known = Known::default();
        assert!(known.bind("alice"));
        known.set("b/1".into(), true);
        known.set("b/2".into(), false);
        assert_eq!(known.get("b/1"), Some(true));
        assert_eq!(known.get("b/2"), Some(false));
        assert_eq!(known.get("b/3"), None);

        assert!(!known.bind("alice"), "same member: nothing to drop");
        assert_eq!(known.get("b/1"), Some(true));

        // Someone else signs in on this browser: none of it carries over.
        assert!(known.bind("bob"));
        assert_eq!(known.get("b/1"), None);
        assert_eq!(known.get("b/2"), None);
    }

    #[test]
    fn survives_being_saved_and_loaded() {
        let mut known = Known::default();
        known.bind("alice");
        known.set("b/1".into(), true);
        known.set("b/2".into(), false);

        let loaded = Known::from_stored(known.to_stored());
        assert_eq!(loaded.owner, "alice");
        assert_eq!(loaded.get("b/1"), Some(true));
        assert_eq!(loaded.get("b/2"), Some(false));
    }

    #[test]
    fn is_bounded_and_keeps_yeses_over_nos() {
        let mut known = Known::default();
        known.set("mine/1".into(), true);
        for i in 0..(MAX_ENTRIES + 50) {
            known.set(format!("b/{i}"), false);
        }
        assert_eq!(known.answers.len(), MAX_ENTRIES);
        assert_eq!(known.get("mine/1"), Some(true), "a yes outlives a flood of nos");
        assert_eq!(known.get("b/0"), None, "the oldest no went first");
        assert_eq!(known.get(&format!("b/{}", MAX_ENTRIES + 49)), Some(false));
    }

    #[test]
    fn an_answer_can_change_without_duplicating() {
        let mut known = Known::default();
        known.set("b/1".into(), false);
        known.set("b/1".into(), true);
        assert_eq!(known.get("b/1"), Some(true));
        assert_eq!(known.order.len(), 1);
    }
}

// ---- for components ---------------------------------------------------------------------

use yew::prelude::*;

use crate::{ApiContext, MeContext};

/// Changes whenever something is learned, so what depends on it is drawn again.
#[derive(Clone, PartialEq)]
pub struct AuthoredContext {
    pub rev: u32,
}

#[derive(Default, PartialEq)]
struct Counter(u32);

impl Reducible for Counter {
    type Action = ();

    fn reduce(self: std::rc::Rc<Self>, _: ()) -> std::rc::Rc<Self> {
        std::rc::Rc::new(Counter(self.0 + 1))
    }
}

#[derive(Properties, PartialEq)]
pub struct ProviderProps {
    pub children: Html,
}

#[function_component]
pub fn AuthoredProvider(props: &ProviderProps) -> Html {
    let rev = use_reducer(Counter::default);
    let me = use_context::<MeContext>().and_then(|m| m.me);

    {
        let dispatcher = rev.dispatcher();
        use_effect_with((), move |_| {
            set_notifier(Callback::from(move |_| dispatcher.dispatch(())));
        });
    }
    // Tied to whoever is signed in; a different person means a clean slate.
    use_effect_with(me.as_ref().map(|m| m.cache_key.clone()), |key| {
        if let Some(key) = key {
            bind_owner(key);
        }
    });

    html! {
        <ContextProvider<AuthoredContext> context={AuthoredContext { rev: rev.0 }}>
            { props.children.clone() }
        </ContextProvider<AuthoredContext>>
    }
}

/// Lookups for a component, which also redraws it when the answers change.
pub struct Authored {
    pub rev: u32,
    api: Option<Arc<Api>>,
}

impl Authored {
    pub fn is_mine(&self, board: &str, number: i64) -> Option<bool> {
        lookup(board, number)
    }

    /// Ask about whatever is not known yet.
    pub fn want(&self, posts: impl IntoIterator<Item = (String, i64)>) {
        if let Some(api) = &self.api {
            want(Arc::clone(api), posts);
        }
    }
}

#[hook]
pub fn use_authored() -> Authored {
    let rev = use_context::<AuthoredContext>().map_or(0, |c| c.rev);
    let api = use_context::<Option<ApiContext>>()
        .flatten()
        .and_then(|c| c.api.ok());
    Authored { rev, api }
}
