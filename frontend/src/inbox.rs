//! The member's notification inbox, and the knowledge of what they are looking at.
//!
//! Which posts are unread is the server's to say (see `get_notifications`); this keeps a copy
//! fresh, turns new arrivals into short-lived toasts, and works out whether the member is
//! how closely the member is paying attention, which every notification path consults before
//! bothering them. There are three tiers, and the server applies the same ones:
//!
//! * **reading the thread**: in the tab they touched most recently, which is visible, shows
//!   that thread's full thread view, and was touched within the last couple of minutes.
//!   Nothing is shown for it at all.
//! * **active on the site**: that tab was touched within the last ten minutes, on any page.
//!   They see notifications in the page; no push is sent to their devices.
//! * **away**: anything else. Pushes go out.
//!
//! Other tabs of the same browser are asked through a `BroadcastChannel`, and the server is
//! told (as a presence report that expires on its own) so it can hold back pushes to their
//! other devices.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};

use common::structs::{Presence, PushMessage, SafePost, ThreadNotification};
use gloo::events::EventListener;
use gloo_timers::callback::Interval;
use wasm_bindgen::{JsCast, JsValue};
use yew::prelude::*;
use yew_hooks::use_interval;
use yew_router::prelude::use_route;

use crate::{
    api::Api,
    authored::{self, use_authored},
    events::use_events,
    helpers::{reply_notifications_enabled, reply_targets},
    ApiContext, BaseRoute, MeContext,
};

/// Without touching the page this long, a thread on screen stops counting as being read. The
/// server applies the same limit (and its own, for using the site at all) from the reports.
const THREAD_IDLE_MS: f64 = 2.0 * 60.0 * 1000.0;
/// Quiet this long before a touch, and that touch is reported at once.
const TOUCH_REPORT_AFTER_MS: f64 = 3_000.0;
/// Past this, the server already treats the member as away, so heartbeats stop.
const GIVE_UP_AFTER_MS: f64 = 10.0 * 60.0 * 1000.0;
/// How often a tab repeats what it is looking at. Must stay well inside the server's
/// `PRESENCE_TTL` (45s), and gives other tabs the same window.
const HEARTBEAT_MS: u32 = 20_000;
const PEER_TTL_MS: f64 = 45_000.0;
/// Waiting this long lets a burst of events become one request.
const REFRESH_DEBOUNCE_MS: u32 = 250;
const MAX_TOASTS: usize = 3;

pub type ThreadKey = (String, i64);

fn key_of(n: &ThreadNotification) -> ThreadKey {
    (n.board_discriminator.clone(), n.thread_post_number)
}

#[derive(Clone, PartialEq, Debug)]
pub struct Toast {
    pub id: u64,
    pub item: ThreadNotification,
    /// A reply to the member's own post, which is not tied to a watched thread.
    pub reply: bool,
}

/// Most replies kept in the list.
const MAX_REPLIES: usize = 50;

/// A reply is identified by its board and post number.
pub type ReplyKey = (String, i64);

#[derive(Clone, PartialEq, Default)]
pub struct InboxState {
    pub items: Vec<ThreadNotification>,
    /// Replies to the member's own posts, newest first. Kept in this browser only.
    pub replies: Vec<ThreadNotification>,
    pub toasts: Vec<Toast>,
    next_toast: u64,
    /// Newest post number seen per thread, to tell what is new from what was already there.
    seen: HashMap<ThreadKey, i64>,
    loaded: bool,
}

impl InboxState {
    /// Total unread posts across every thread.
    /// Whether the first copy of the list has arrived, which is also when we know the member
    /// is signed in and the inbox is worth drawing.
    pub fn loaded(&self) -> bool {
        self.loaded
    }

    pub fn unread_total(&self) -> i64 {
        self.items.iter().map(|i| i.unread).sum::<i64>() + self.replies.len() as i64
    }
}

pub enum InboxAction {
    /// A fresh copy of the unread list. `suppressed` are threads being looked at right now.
    Loaded {
        items: Vec<ThreadNotification>,
        suppressed: HashSet<ThreadKey>,
    },
    DismissToast(u64),
    /// A post that replies to one of the member's. `toast` is false for ones found by catching
    /// up, which are news to the list but not to the screen.
    ReplyArrived {
        note: ThreadNotification,
        toast: bool,
    },
    DismissReply(ReplyKey),
    /// The member is reading this thread, so replies in it have been seen.
    ThreadRead(ThreadKey),
    SetReplies(Vec<ThreadNotification>),
}

impl Reducible for InboxState {
    type Action = InboxAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        let mut next = (*self).clone();
        match action {
            InboxAction::DismissToast(id) => next.toasts.retain(|t| t.id != id),
            InboxAction::SetReplies(replies) => next.replies = replies,
            InboxAction::DismissReply((board, number)) => {
                next.replies
                    .retain(|n| !(n.board_discriminator == board && n.latest.post_number == number));
                next.toasts.retain(|t| {
                    !(t.reply && t.item.board_discriminator == board && t.item.latest.post_number == number)
                });
            }
            InboxAction::ThreadRead((board, thread)) => {
                next.replies
                    .retain(|n| !(n.board_discriminator == board && n.thread_post_number == thread));
                next.toasts.retain(|t| {
                    !(t.reply && t.item.board_discriminator == board && t.item.thread_post_number == thread)
                });
            }
            InboxAction::ReplyArrived { note, toast } => {
                let same = |n: &ThreadNotification| {
                    n.board_discriminator == note.board_discriminator
                        && n.latest.post_number == note.latest.post_number
                };
                // The live event and the catch-up can both report the same post.
                if !next.replies.iter().any(same) {
                    next.replies.insert(0, note.clone());
                    next.replies.truncate(MAX_REPLIES);
                    if toast {
                        let id = next.next_toast;
                        next.next_toast += 1;
                        next.toasts.push(Toast {
                            id,
                            item: note,
                            reply: true,
                        });
                        while next.toasts.len() > MAX_TOASTS {
                            next.toasts.remove(0);
                        }
                    }
                }
            }
            InboxAction::Loaded { items, suppressed } => {
                // A toast is a new arrival in the server's list that nobody is already
                // looking at. Deriving it from the list, rather than from the raw event,
                // means it only ever appears for threads that are watched and really unread.
                // The first load is not news: it is what was waiting.
                if next.loaded {
                    for item in &items {
                        let key = key_of(item);
                        let is_new = next
                            .seen
                            .get(&key)
                            .map_or(true, |seen| item.latest.post_number > *seen);
                        if is_new && !suppressed.contains(&key) {
                            next.toasts.retain(|t| t.reply || key_of(&t.item) != key);
                            let id = next.next_toast;
                            next.next_toast += 1;
                            next.toasts.push(Toast {
                                id,
                                item: item.clone(),
                                reply: false,
                            });
                            while next.toasts.len() > MAX_TOASTS {
                                next.toasts.remove(0);
                            }
                        }
                    }
                }
                next.seen = items
                    .iter()
                    .map(|i| (key_of(i), i.latest.post_number))
                    .collect();
                // Once a thread has been read (or looked at), its toast has nothing to say.
                let still_unread: HashSet<ThreadKey> = items.iter().map(key_of).collect();
                next.toasts
                    .retain(|t| t.reply || still_unread.contains(&key_of(&t.item)));
                next.items = items;
                next.loaded = true;
            }
        }
        Rc::new(next)
    }
}

#[derive(Clone, PartialEq)]
pub struct InboxContext {
    pub state: InboxState,
    /// The full thread view being looked at right now, if any.
    pub active: Option<ThreadKey>,
    pub dismiss_toast: Callback<u64>,
    /// (board, thread, post number read through)
    pub mark_read: Callback<(String, i64, i64)>,
    pub mark_all_read: Callback<()>,
    pub stop_watching: Callback<ThreadKey>,
    pub dismiss_reply: Callback<ReplyKey>,
}

#[hook]
pub fn use_inbox() -> Option<InboxContext> {
    use_context::<InboxContext>()
}

fn document_visible() -> bool {
    let document = gloo::utils::document();
    js_sys::Reflect::get(&document, &"visibilityState".into())
        .ok()
        .and_then(|v| v.as_string())
        .map_or(true, |s| s == "visible")
}

fn random_tab_id() -> String {
    // The page makes one up before the app starts, so its goodbye on close can name this tab.
    if let Some(id) = web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w, &"pchanTabId".into()).ok())
        .and_then(|v| v.as_string())
    {
        return id;
    }
    format!(
        "{:x}{:x}",
        (js_sys::Math::random() * 1e15) as u64,
        js_sys::Date::now() as u64
    )
}

/// What a tab says about itself on the `BroadcastChannel`.
#[derive(serde::Serialize, serde::Deserialize)]
struct PeerReport {
    tab: String,
    thread: Option<ThreadKey>,
}

type Peers = Rc<RefCell<HashMap<String, (ThreadKey, f64)>>>;

fn peer_is_viewing(peers: &Peers, key: &ThreadKey) -> bool {
    let now = js_sys::Date::now();
    peers
        .borrow()
        .values()
        .any(|(thread, until)| thread == key && *until > now)
}

type Pending = Rc<RefCell<Vec<(SafePost, bool)>>>;

/// Whether a post replies to one of the member's own, as far as this browser can tell.
enum Verdict {
    ToMe,
    NotToMe,
    /// Some of what it replies to is not known yet.
    Unknown(Vec<(String, i64)>),
}

fn verdict(post: &SafePost) -> Verdict {
    // The member's own post, which can arrive back to them, is not a reply to them.
    if authored::lookup(&post.board_discriminator, post.post_number) == Some(true) {
        return Verdict::NotToMe;
    }
    let targets = reply_targets(&post.content, &post.board_discriminator, post.thread_post_number);
    let mut unknown = Vec::new();
    for (board, number) in targets {
        match authored::lookup(&board, number) {
            Some(true) => return Verdict::ToMe,
            Some(false) => {}
            None => unknown.push((board, number)),
        }
    }
    if unknown.is_empty() {
        Verdict::NotToMe
    } else {
        Verdict::Unknown(unknown)
    }
}

/// A reply drawn like any other notification card. The topic is not known from a post alone, so
/// it names the thread instead.
fn reply_note(post: &SafePost) -> ThreadNotification {
    ThreadNotification {
        board_discriminator: post.board_discriminator.clone(),
        thread_post_number: post.thread_post_number,
        topic: format!("/{}/ thread {}", post.board_discriminator, post.thread_post_number),
        unread: 1,
        first_unread: post.post_number,
        latest: post.clone(),
    }
}

const REPLY_NOTES_KEY: &str = "reply_notes";
const REPLY_CURSOR_KEY: &str = "reply_cursor";

/// Replies kept for one member. Tagged with whose they are, so they are dropped, not shown,
/// if someone else signs in on this browser.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct StoredReplies {
    owner: String,
    notes: Vec<ThreadNotification>,
}

fn load_reply_notes() -> StoredReplies {
    use gloo::storage::{LocalStorage, Storage};
    LocalStorage::get::<StoredReplies>(REPLY_NOTES_KEY).unwrap_or_default()
}

fn save_reply_notes(owner: &str, notes: &[ThreadNotification]) {
    use gloo::storage::{LocalStorage, Storage};
    let _ = LocalStorage::set(
        REPLY_NOTES_KEY,
        StoredReplies {
            owner: owner.to_owned(),
            notes: notes.to_vec(),
        },
    );
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct StoredCursor {
    owner: String,
    cursor: i64,
}

/// Where catching up left off, or `None` for a member who has not caught up before.
fn load_reply_cursor(owner: &str) -> Option<i64> {
    use gloo::storage::{LocalStorage, Storage};
    LocalStorage::get::<StoredCursor>(REPLY_CURSOR_KEY)
        .ok()
        .filter(|c| c.owner == owner)
        .map(|c| c.cursor)
}

fn save_reply_cursor(owner: &str, cursor: i64) {
    use gloo::storage::{LocalStorage, Storage};
    let _ = LocalStorage::set(
        REPLY_CURSOR_KEY,
        StoredCursor {
            owner: owner.to_owned(),
            cursor,
        },
    );
}

#[derive(Properties, PartialEq)]
pub struct ProviderProps {
    pub children: Html,
}

#[function_component]
pub fn InboxProvider(props: &ProviderProps) -> Html {
    let signed_in = use_context::<MeContext>()
        .map(|m| m.me.is_some())
        .unwrap_or(false);
    let api: Option<Arc<Api>> = use_context::<Option<ApiContext>>()
        .flatten()
        .and_then(|c| c.api.ok());
    let route = use_route::<BaseRoute>();

    let state = use_reducer(|| InboxState {
        replies: load_reply_notes().notes,
        ..Default::default()
    });

    // ---- are they looking? -------------------------------------------------------------
    //
    // This tab reports facts to the server (visible, which thread it shows, how long since it
    // was touched) and the server decides what they add up to. The same facts drive this tab's
    // own decisions: whether a toast would be redundant, and whether reading counts.

    let visible = use_state(document_visible);
    let thread_idle = use_state(|| false);
    let last_activity = use_mut_ref(js_sys::Date::now);
    let idle_flag = use_mut_ref(|| false);
    let tab_id = use_state(random_tab_id);
    // Sends this tab's report straight away. Held in a cell so the input listeners, which are
    // set up once, always call the current one.
    let send_now = use_mut_ref(Callback::<()>::noop);

    {
        let visible = visible.setter();
        use_effect_with((), move |_| {
            let document = gloo::utils::document();
            let listener = EventListener::new(&document, "visibilitychange", move |_| {
                visible.set(document_visible());
            });
            move || drop(listener)
        });
    }
    {
        let last_activity = Rc::clone(&last_activity);
        let idle_flag = Rc::clone(&idle_flag);
        let send_now = Rc::clone(&send_now);
        let thread_idle = thread_idle.setter();
        use_effect_with((), move |_| {
            let window = gloo::utils::window();
            let listeners: Vec<EventListener> = [
                "mousemove",
                "mousedown",
                "keydown",
                "wheel",
                "scroll",
                "touchstart",
            ]
            .into_iter()
            .map(|name| {
                let last_activity = Rc::clone(&last_activity);
                let idle_flag = Rc::clone(&idle_flag);
                let send_now = Rc::clone(&send_now);
                let thread_idle = thread_idle.clone();
                EventListener::new(&window, name, move |_| {
                    let now = js_sys::Date::now();
                    let was_quiet = now - *last_activity.borrow() >= TOUCH_REPORT_AFTER_MS;
                    *last_activity.borrow_mut() = now;
                    // Coming back counts at once, not at the next tick.
                    if idle_flag.replace(false) {
                        thread_idle.set(false);
                    }
                    // The first touch after a quiet spell tells the server right away, so
                    // attention moves to this tab the moment it is picked up, whichever tab it
                    // was in before. Touches in quick succession do not each send one.
                    if was_quiet {
                        let send = send_now.borrow().clone();
                        send.emit(());
                    }
                })
            })
            .collect();
            move || drop(listeners)
        });
    }
    {
        let last_activity = Rc::clone(&last_activity);
        let idle_flag = Rc::clone(&idle_flag);
        let thread_idle = thread_idle.setter();
        use_interval(
            move || {
                let quiet = js_sys::Date::now() - *last_activity.borrow() > THREAD_IDLE_MS;
                if quiet != idle_flag.replace(quiet) {
                    thread_idle.set(quiet);
                }
            },
            10_000,
        );
    }

    let route_key: Option<ThreadKey> = match route {
        Some(BaseRoute::ThreadPage {
            board_discriminator,
            thread_id,
        }) => thread_id.parse::<i64>().ok().map(|n| (board_discriminator, n)),
        _ => None,
    };
    // Reading, as far as this tab can tell on its own.
    let active: Option<ThreadKey> = route_key.clone().filter(|_| *visible && !*thread_idle);

    let report_presence = {
        let api = api.clone();
        let tab = (*tab_id).clone();
        let last_activity = Rc::clone(&last_activity);
        let visible = *visible;
        let showing = route_key.clone();
        Callback::from(move |_: ()| {
            let (true, Some(api)) = (signed_in, api.clone()) else {
                return;
            };
            let quiet = (js_sys::Date::now() - *last_activity.borrow()).max(0.0) / 1000.0;
            let presence = Presence {
                tab: tab.clone(),
                visible,
                reading: showing.as_ref().map(|(b, n)| format!("{b}/{n}")),
                quiet_secs: quiet as u32,
            };
            wasm_bindgen_futures::spawn_local(async move {
                let _ = api.report_presence(&presence).await;
            });
        })
    };
    *send_now.borrow_mut() = report_presence.clone();

    // The server's picture changes when this tab changes: shown or hidden, a different page,
    // signed in or out. Then it is repeated while the tab is visible, so a report does not
    // lapse under someone who is still there. Going quiet needs no report of its own: the
    // server counts the time from when the tab was last touched.
    {
        let last_activity = Rc::clone(&last_activity);
        use_effect_with(
            (*visible, route_key.clone(), signed_in),
            move |(visible, _, signed_in)| {
                report_presence.emit(());
                let heartbeat = (*visible && *signed_in).then(|| {
                    Interval::new(HEARTBEAT_MS, move || {
                        // Long past quiet, the server already treats them as away.
                        if js_sys::Date::now() - *last_activity.borrow() < GIVE_UP_AFTER_MS {
                            report_presence.emit(());
                        }
                    })
                });
                move || drop(heartbeat)
            },
        );
    }

    // Other tabs of this browser, so a toast is not shown here for a thread being read there.
    let peers: Peers = use_mut_ref(HashMap::new);
    let channel = use_mut_ref(|| None::<web_sys::BroadcastChannel>);
    {
        let peers = Rc::clone(&peers);
        let channel = Rc::clone(&channel);
        let tab_id = (*tab_id).clone();
        use_effect_with((), move |_| {
            let bc = web_sys::BroadcastChannel::new("pchan-viewing").ok();
            let listener = bc.as_ref().map(|bc| {
                EventListener::new(bc, "message", move |event| {
                    let Some(message) = event.dyn_ref::<web_sys::MessageEvent>() else {
                        return;
                    };
                    let Some(text) = message.data().as_string() else {
                        return;
                    };
                    let Ok(report) = serde_json::from_str::<PeerReport>(&text) else {
                        return;
                    };
                    if report.tab == tab_id {
                        return;
                    }
                    let mut peers = peers.borrow_mut();
                    match report.thread {
                        Some(thread) => {
                            peers.insert(report.tab, (thread, js_sys::Date::now() + PEER_TTL_MS));
                        }
                        None => {
                            peers.remove(&report.tab);
                        }
                    }
                })
            });
            *channel.borrow_mut() = bc.clone();
            move || {
                drop(listener);
                if let Some(bc) = bc {
                    bc.close();
                }
            }
        });
    }
    {
        let channel = Rc::clone(&channel);
        let tab = (*tab_id).clone();
        use_effect_with(active.clone(), move |active| {
            let say = {
                let active = active.clone();
                move || {
                    if let Some(bc) = channel.borrow().as_ref() {
                        let peer = PeerReport {
                            tab: tab.clone(),
                            thread: active.clone(),
                        };
                        if let Ok(text) = serde_json::to_string(&peer) {
                            let _ = bc.post_message(&JsValue::from_str(&text));
                        }
                    }
                }
            };
            say();
            let heartbeat = active
                .is_some()
                .then(|| Interval::new(HEARTBEAT_MS, move || say()));
            move || drop(heartbeat)
        });
    }

    // ---- keeping the inbox fresh --------------------------------------------------------

    let active_now = use_mut_ref(|| None::<ThreadKey>);
    *active_now.borrow_mut() = active.clone();
    let generation = use_mut_ref(|| 0u32);

    let refresh = {
        let api = api.clone();
        let dispatcher = state.dispatcher();
        let peers = Rc::clone(&peers);
        let active_now = Rc::clone(&active_now);
        let generation = Rc::clone(&generation);
        Callback::from(move |_: ()| {
            let Some(api) = api.clone() else {
                return;
            };
            let mine = {
                let mut g = generation.borrow_mut();
                *g += 1;
                *g
            };
            let (dispatcher, peers, active_now, generation) = (
                dispatcher.clone(),
                Rc::clone(&peers),
                Rc::clone(&active_now),
                Rc::clone(&generation),
            );
            wasm_bindgen_futures::spawn_local(async move {
                gloo_timers::future::TimeoutFuture::new(REFRESH_DEBOUNCE_MS).await;
                if *generation.borrow() != mine {
                    return; // a newer request has replaced this one
                }
                match api.get_notifications().await {
                    Ok(items) => {
                        // Worked out when the answer arrives, not when it was asked for.
                        let mut suppressed = HashSet::new();
                        for item in &items {
                            let key = key_of(item);
                            if active_now.borrow().as_ref() == Some(&key)
                                || peer_is_viewing(&peers, &key)
                            {
                                suppressed.insert(key);
                            }
                        }
                        dispatcher.dispatch(InboxAction::Loaded { items, suppressed });
                    }
                    Err(e) => gloo::console::warn!(format!("could not load notifications: {}", *e)),
                }
            });
        })
    };

    {
        let refresh = refresh.clone();
        use_effect_with(signed_in, move |signed_in| {
            if *signed_in {
                refresh.emit(());
            }
        });
    }
    // ---- replies to the member's own posts -----------------------------------------------
    //
    // Every reply reaches every browser that can see the board, so the server never has to
    // know who a reply is for. This browser works that out from what it remembers about which
    // posts are the member's (see `authored`), asking the server about any it does not know.

    let owner = use_context::<MeContext>()
        .and_then(|m| m.me)
        .map(|m| m.cache_key);
    let authored = use_authored();
    let pending: Pending = use_mut_ref(Vec::new);

    let handle_reply = {
        let dispatcher = state.dispatcher();
        let active_now = Rc::clone(&active_now);
        let peers = Rc::clone(&peers);
        let pending = Rc::clone(&pending);
        let api = api.clone();
        Callback::from(move |(post, toast): (SafePost, bool)| match verdict(&post) {
            Verdict::NotToMe => {}
            Verdict::ToMe => {
                // Reading that thread: it is on screen, so there is nothing to announce.
                let key = (post.board_discriminator.clone(), post.thread_post_number);
                let looking = active_now.borrow().as_ref() == Some(&key)
                    || peer_is_viewing(&peers, &key);
                if !looking {
                    dispatcher.dispatch(InboxAction::ReplyArrived {
                        note: reply_note(&post),
                        toast,
                    });
                }
            }
            Verdict::Unknown(unknown) => {
                pending.borrow_mut().push((post, toast));
                if let Some(api) = api.clone() {
                    authored::want(api, unknown);
                }
            }
        })
    };

    // What was waiting on an answer from the server, once the answers arrive.
    {
        let pending = Rc::clone(&pending);
        let handle_reply = handle_reply.clone();
        use_effect_with(authored.rev, move |_| {
            let waiting: Vec<_> = pending.borrow_mut().drain(..).collect();
            for entry in waiting {
                handle_reply.emit(entry);
            }
        });
    }

    // Replies made while this browser was not listening: closed, asleep, or disconnected.
    let catching_up = use_mut_ref(|| false);
    let catch_up = {
        let api = api.clone();
        let handle_reply = handle_reply.clone();
        let catching_up = Rc::clone(&catching_up);
        let owner = owner.clone();
        Callback::from(move |_: ()| {
            let (Some(api), Some(owner)) = (api.clone(), owner.clone()) else {
                return;
            };
            if !reply_notifications_enabled() || catching_up.replace(true) {
                return;
            }
            let (handle_reply, catching_up) = (handle_reply.clone(), Rc::clone(&catching_up));
            wasm_bindgen_futures::spawn_local(async move {
                loop {
                    match api.get_replies(load_reply_cursor(&owner)).await {
                        Ok(page) => {
                            for post in page.posts {
                                handle_reply.emit((post, false));
                            }
                            save_reply_cursor(&owner, page.cursor);
                            if !page.more {
                                break;
                            }
                        }
                        Err(e) => {
                            gloo::console::warn!(format!("could not catch up on replies: {}", *e));
                            break;
                        }
                    }
                }
                *catching_up.borrow_mut() = false;
            });
        })
    };
    {
        let catch_up = catch_up.clone();
        use_effect_with(owner.clone(), move |owner| {
            if owner.is_some() {
                catch_up.emit(());
            }
        });
    }

    {
        let refresh = refresh.clone();
        let handle_reply = handle_reply.clone();
        let catch_up = catch_up.clone();
        use_events(Callback::from(move |message: PushMessage| {
            match &message {
                PushMessage::NewPost(post) if reply_notifications_enabled() => {
                    handle_reply.emit(((**post).clone(), true));
                }
                // Back in touch after a gap: pick up what was missed.
                PushMessage::Open | PushMessage::Resync => catch_up.emit(()),
                _ => {}
            }
            match message {
                PushMessage::NewPost(_)
                | PushMessage::PostDeleted(_)
                | PushMessage::InboxChanged
                | PushMessage::Resync
                | PushMessage::Open => refresh.emit(()),
            }
        }));
    }

    // Replies kept in this browser belong to this member only, and are saved as they change.
    {
        let dispatcher = state.dispatcher();
        use_effect_with(owner.clone(), move |owner| {
            if let Some(owner) = owner {
                if load_reply_notes().owner != *owner {
                    dispatcher.dispatch(InboxAction::SetReplies(vec![]));
                    save_reply_notes(owner, &[]);
                }
            }
        });
    }
    {
        let owner = owner.clone();
        use_effect_with(state.replies.clone(), move |replies| {
            if let Some(owner) = &owner {
                save_reply_notes(owner, replies);
            }
        });
    }
    // Reading a thread is seeing what replied in it.
    {
        let dispatcher = state.dispatcher();
        use_effect_with(active.clone(), move |active| {
            if let Some(key) = active {
                dispatcher.dispatch(InboxAction::ThreadRead(key.clone()));
            }
        });
    }

    // The tab title carries the count, so it is noticed from another tab.
    {
        let unread = state.unread_total();
        use_effect_with(unread, move |unread| {
            if let Some(window) = web_sys::window() {
                if let Ok(f) = js_sys::Reflect::get(&window, &"pchanSetUnread".into()) {
                    if let Ok(f) = f.dyn_into::<js_sys::Function>() {
                        let _ = f.call1(&window, &JsValue::from_f64(*unread as f64));
                    }
                }
            }
        });
    }

    // ---- what components can ask for ----------------------------------------------------

    let dismiss_toast = {
        let dispatcher = state.dispatcher();
        Callback::from(move |id: u64| dispatcher.dispatch(InboxAction::DismissToast(id)))
    };
    // Each action refreshes the list itself when the server has answered. The server also
    // announces the change to every tab and device, which is what updates the others, but
    // this one should not depend on that message arriving to show what the member just did.
    let mark_read = {
        let api = api.clone();
        let refresh = refresh.clone();
        Callback::from(move |(board, thread, through): (String, i64, i64)| {
            if let Some(api) = api.clone() {
                let refresh = refresh.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Err(e) = api.mark_thread_read(&board, thread, through).await {
                        gloo::console::warn!(format!("could not mark thread read: {}", *e));
                    }
                    refresh.emit(());
                });
            }
        })
    };
    let mark_all_read = {
        let api = api.clone();
        let refresh = refresh.clone();
        Callback::from(move |_: ()| {
            if let Some(api) = api.clone() {
                let refresh = refresh.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Err(e) = api.mark_all_read().await {
                        gloo::console::warn!(format!("could not mark all read: {}", *e));
                    }
                    refresh.emit(());
                });
            }
        })
    };
    let stop_watching = {
        let api = api;
        let refresh = refresh;
        Callback::from(move |(board, thread): ThreadKey| {
            if let Some(api) = api.clone() {
                let refresh = refresh.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Err(e) = api.set_watching(&board, thread, false).await {
                        gloo::console::warn!(format!("could not stop watching: {}", *e));
                    }
                    refresh.emit(());
                });
            }
        })
    };

    let dismiss_reply = {
        let dispatcher = state.dispatcher();
        Callback::from(move |key: ReplyKey| dispatcher.dispatch(InboxAction::DismissReply(key)))
    };

    let context = InboxContext {
        state: (*state).clone(),
        active,
        dismiss_toast,
        mark_read,
        mark_all_read,
        stop_watching,
        dismiss_reply,
    };
    html! {
        <ContextProvider<InboxContext> context={context}>
            { props.children.clone() }
        </ContextProvider<InboxContext>>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(board: &str, thread: i64, latest: i64, unread: i64) -> ThreadNotification {
        serde_json::from_value(serde_json::json!({
            "board_discriminator": board,
            "thread_post_number": thread,
            "topic": "topic",
            "unread": unread,
            "first_unread": latest,
            "latest": {
                "post_number": latest, "file": null, "thread_post_number": thread,
                "board_discriminator": board, "author": "Anonymous", "content": "hi",
                "timestamp": "2026-01-01T00:00:00Z", "replies": []
            }
        }))
        .unwrap()
    }

    fn load(state: Rc<InboxState>, items: Vec<ThreadNotification>, suppressed: &[ThreadKey]) -> Rc<InboxState> {
        state.reduce(InboxAction::Loaded {
            items,
            suppressed: suppressed.iter().cloned().collect(),
        })
    }

    fn fresh() -> Rc<InboxState> {
        Rc::new(InboxState::default())
    }

    fn key(board: &str, thread: i64) -> ThreadKey {
        (board.to_owned(), thread)
    }

    #[test]
    fn what_was_waiting_on_arrival_is_not_news() {
        let state = load(fresh(), vec![item("b", 1, 5, 3)], &[]);
        assert!(state.toasts.is_empty());
        assert_eq!(state.unread_total(), 3);
        assert!(state.loaded());
    }

    #[test]
    fn a_newer_post_in_the_list_becomes_a_toast() {
        let state = load(fresh(), vec![item("b", 1, 5, 1)], &[]);
        let state = load(state, vec![item("b", 1, 6, 2)], &[]);
        assert_eq!(state.toasts.len(), 1);
        assert_eq!(state.toasts[0].item.latest.post_number, 6);

        // The same list again is not new.
        let again = load(state.clone(), vec![item("b", 1, 6, 2)], &[]);
        assert_eq!(again.toasts.len(), 1);
        assert_eq!(again.toasts[0].id, state.toasts[0].id);
    }

    #[test]
    fn a_thread_being_read_never_toasts_but_still_updates_the_list() {
        let state = load(fresh(), vec![], &[]);
        let state = load(state, vec![item("b", 1, 6, 1)], &[key("b", 1)]);
        assert!(state.toasts.is_empty());
        assert_eq!(state.items.len(), 1);
    }

    #[test]
    fn a_thread_replaces_its_own_toast_instead_of_stacking() {
        let state = load(fresh(), vec![], &[]);
        let state = load(state, vec![item("b", 1, 6, 1)], &[]);
        let state = load(state, vec![item("b", 1, 7, 2)], &[]);
        assert_eq!(state.toasts.len(), 1);
        assert_eq!(state.toasts[0].item.latest.post_number, 7);
    }

    #[test]
    fn only_the_latest_few_toasts_are_kept() {
        let mut state = load(fresh(), vec![], &[]);
        let mut items = Vec::new();
        for t in 1..=(MAX_TOASTS as i64 + 2) {
            items.push(item("b", t, 10, 1));
        }
        state = load(state, items, &[]);
        assert_eq!(state.toasts.len(), MAX_TOASTS);
    }

    #[test]
    fn reading_a_thread_clears_its_toast_and_a_later_post_toasts_again() {
        let state = load(fresh(), vec![], &[]);
        let state = load(state, vec![item("b", 1, 6, 1)], &[]);
        assert_eq!(state.toasts.len(), 1);

        // Read elsewhere: gone from the list, so the toast has nothing left to say.
        let state = load(state, vec![], &[]);
        assert!(state.toasts.is_empty());

        // A new post after that is news again.
        let state = load(state, vec![item("b", 1, 9, 1)], &[]);
        assert_eq!(state.toasts.len(), 1);
    }

    #[test]
    fn dismissing_a_toast_does_not_touch_the_list() {
        let state = load(fresh(), vec![], &[]);
        let state = load(state, vec![item("b", 1, 6, 1)], &[]);
        let id = state.toasts[0].id;
        let state = state.reduce(InboxAction::DismissToast(id));
        assert!(state.toasts.is_empty());
        assert_eq!(state.items.len(), 1);
    }

    fn reply(board: &str, thread: i64, post: i64) -> ThreadNotification {
        item(board, thread, post, 1)
    }

    fn arrive(state: Rc<InboxState>, note: ThreadNotification, toast: bool) -> Rc<InboxState> {
        state.reduce(InboxAction::ReplyArrived { note, toast })
    }

    #[test]
    fn a_reply_is_listed_once_even_if_reported_twice() {
        let state = arrive(fresh(), reply("b", 1, 9), true);
        let state = arrive(state, reply("b", 1, 9), true);
        assert_eq!(state.replies.len(), 1);
        assert_eq!(state.toasts.len(), 1, "the repeat made no second toast");
        assert_eq!(state.unread_total(), 1, "replies count toward the bell");
    }

    #[test]
    fn caught_up_replies_are_listed_without_a_toast() {
        let state = arrive(fresh(), reply("b", 1, 9), false);
        assert_eq!(state.replies.len(), 1);
        assert!(state.toasts.is_empty());
    }

    #[test]
    fn a_reply_toast_outlives_a_reload_of_the_watched_list() {
        let state = load(fresh(), vec![], &[]);
        let state = arrive(state, reply("b", 7, 30), true);
        // The thread is not watched, so it is never in the server's list.
        let state = load(state, vec![], &[]);
        assert_eq!(state.toasts.len(), 1);
        assert_eq!(state.replies.len(), 1);
    }

    #[test]
    fn newest_replies_come_first_and_the_list_is_bounded() {
        let mut state = fresh();
        for post in 1..=(MAX_REPLIES as i64 + 5) {
            state = arrive(state, reply("b", 1, post), false);
        }
        assert_eq!(state.replies.len(), MAX_REPLIES);
        assert_eq!(state.replies[0].latest.post_number, MAX_REPLIES as i64 + 5);
    }

    #[test]
    fn dismissing_or_reading_removes_replies_and_their_toasts() {
        let state = arrive(fresh(), reply("b", 1, 9), true);
        let state = arrive(state, reply("b", 2, 12), true);
        let state = arrive(state, reply("c", 1, 4), true);

        let state = state.reduce(InboxAction::DismissReply(("b".into(), 9)));
        assert_eq!(state.replies.len(), 2);
        assert_eq!(state.toasts.len(), 2);

        // Reading thread b/2 clears what replied in it, and nothing else.
        let state = state.reduce(InboxAction::ThreadRead(("b".into(), 2)));
        assert_eq!(state.replies.len(), 1);
        assert_eq!(state.replies[0].board_discriminator, "c");
        assert_eq!(state.toasts.len(), 1);
    }
}
