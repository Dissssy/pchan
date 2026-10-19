//! The page's one live connection to the server.
//!
//! A single `EventSource` is owned by [`EventsProvider`] and reconnected when the open thread
//! changes. Components that care about events register a callback with [`use_events`]; every
//! event is handed to every callback immediately, in the order it arrived. Nothing is queued
//! in component state, so a burst cannot overwrite itself or be reordered.
//!
//! Recovery lives on the server: each event carries an id, the browser (or this code, when it
//! has to open a fresh connection) sends the last one it saw, and the server either replays
//! what was missed or sends `resync`, which consumers answer by reloading what they show.

use std::{cell::RefCell, rc::Rc, sync::Arc};

use common::structs::{DeletedPost, PushMessage, SafePost};
use futures::{channel::oneshot, FutureExt, StreamExt};
use gloo_net::eventsource::{futures::EventSource, State};
use yew::prelude::*;
use yew_router::prelude::use_route;

use crate::{BaseRoute, MeContext};

const EVENT_TYPES: [&str; 5] = ["open", "new_post", "post_deleted", "resync", "inbox_changed"];
const RETRY_FIRST_MS: u32 = 1_000;
const RETRY_MAX_MS: u32 = 30_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LiveStatus {
    Connecting,
    Live,
    /// The connection dropped and is being retried.
    Paused,
}

type Listeners = Rc<RefCell<Vec<(usize, Callback<PushMessage>)>>>;

#[derive(Clone)]
pub struct EventBus {
    listeners: Listeners,
    next_id: Rc<RefCell<usize>>,
    pub status: LiveStatus,
}

impl PartialEq for EventBus {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.listeners, &other.listeners) && self.status == other.status
    }
}

/// Identity of a bus, ignoring its status, so that registering a listener does not redo
/// itself every time the connection state flickers.
#[derive(Clone)]
struct BusIdentity(Listeners, Rc<RefCell<usize>>);

impl PartialEq for BusIdentity {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Run `callback` for every live event while the calling component is mounted. The most
/// recent callback is always the one that runs, so it may close over current state.
#[hook]
pub fn use_events(callback: Callback<PushMessage>) {
    let bus = use_context::<EventBus>().map(|b| BusIdentity(b.listeners, b.next_id));
    let latest = use_mut_ref(|| callback.clone());
    *latest.borrow_mut() = callback;

    use_effect_with(bus, move |bus| {
        let registration = bus.as_ref().map(|BusIdentity(listeners, next_id)| {
            let id = {
                let mut next = next_id.borrow_mut();
                *next += 1;
                *next
            };
            listeners.borrow_mut().push((
                id,
                Callback::from(move |message: PushMessage| {
                    // Copy the callback out before running it. Running it usually sets
                    // state, which re-renders the component right here, and that render
                    // replaces `latest`: doing so while it is still borrowed panics.
                    let callback = latest.borrow().clone();
                    callback.emit(message);
                }),
            ));
            (Rc::clone(listeners), id)
        });
        move || {
            if let Some((listeners, id)) = registration {
                listeners.borrow_mut().retain(|(i, _)| *i != id);
            }
        }
    });
}

/// Where the live connection currently stands.
#[hook]
pub fn use_live_status() -> LiveStatus {
    use_context::<EventBus>()
        .map(|b| b.status)
        .unwrap_or(LiveStatus::Connecting)
}

#[derive(Properties, PartialEq)]
pub struct ProviderProps {
    pub children: Html,
}

#[function_component]
pub fn EventsProvider(props: &ProviderProps) -> Html {
    let route = use_route::<BaseRoute>();
    let signed_in = use_context::<MeContext>()
        .map(|m| m.me.is_some())
        .unwrap_or(false);

    let listeners: Listeners = use_mut_ref(Vec::new);
    let next_id = use_mut_ref(|| 0usize);
    let last_id = use_mut_ref(|| None::<String>);
    let status = use_state(|| LiveStatus::Connecting);

    let thread = match route {
        Some(BaseRoute::ThreadPage {
            board_discriminator,
            thread_id,
        }) if thread_id.parse::<i64>().is_ok() => Some(format!("{board_discriminator}/{thread_id}")),
        _ => None,
    };

    {
        let listeners = Rc::clone(&listeners);
        let status = status.setter();
        use_effect_with((thread, signed_in), move |(thread, signed_in)| {
            let (cancel, cancelled) = oneshot::channel::<()>();
            if *signed_in {
                wasm_bindgen_futures::spawn_local(run(
                    thread.clone(),
                    listeners,
                    last_id,
                    status,
                    cancelled,
                ));
            }
            move || {
                cancel.send(()).ok();
            }
        });
    }

    let bus = EventBus {
        listeners,
        next_id,
        status: *status,
    };
    html! {
        <ContextProvider<EventBus> context={bus}>
            { props.children.clone() }
        </ContextProvider<EventBus>>
    }
}

fn url_for(thread: &Option<String>, last: &Option<String>) -> String {
    let encode = |s: &str| String::from(js_sys::encode_uri_component(s));
    let mut query = Vec::new();
    if let Some(thread) = thread {
        query.push(format!("thread={}", encode(thread)));
    }
    if let Some(last) = last {
        query.push(format!("last={}", encode(last)));
    }
    if query.is_empty() {
        "/api/v1/events".to_owned()
    } else {
        format!("/api/v1/events?{}", query.join("&"))
    }
}

fn parse(kind: &str, data: Option<String>) -> Option<PushMessage> {
    let decode = |what: &str, data: Option<String>| -> Option<String> {
        if data.is_none() {
            gloo::console::warn!(format!("{what} event without data"));
        }
        data
    };
    match kind {
        "open" => Some(PushMessage::Open),
        "resync" => Some(PushMessage::Resync),
        "inbox_changed" => Some(PushMessage::InboxChanged),
        "new_post" => {
            let data = decode(kind, data)?;
            match serde_json::from_str::<SafePost>(&data) {
                Ok(post) => Some(PushMessage::NewPost(Arc::new(post))),
                Err(e) => {
                    gloo::console::error!(format!("unreadable new_post event: {e}"));
                    None
                }
            }
        }
        "post_deleted" => {
            let data = decode(kind, data)?;
            match serde_json::from_str::<DeletedPost>(&data) {
                Ok(gone) => Some(PushMessage::PostDeleted(Arc::new(gone))),
                Err(e) => {
                    gloo::console::error!(format!("unreadable post_deleted event: {e}"));
                    None
                }
            }
        }
        other => {
            gloo::console::warn!(format!("unknown event type: `{other}`"));
            None
        }
    }
}

/// Keep a connection open until `cancelled` fires, reconnecting with backoff whenever it
/// ends for good.
///
/// The browser retries a dropped connection by itself and resumes from the last event id. What
/// it will not do is retry after the server answers with an error (an expired session, say),
/// which closes the `EventSource` for good; that case is handled here, with a fresh
/// connection that passes the last id by hand.
async fn run(
    thread: Option<String>,
    listeners: Listeners,
    last_id: Rc<RefCell<Option<String>>>,
    status: UseStateSetter<LiveStatus>,
    mut cancelled: oneshot::Receiver<()>,
) {
    let mut failures = 0u32;
    loop {
        let url = url_for(&thread, &last_id.borrow());
        match EventSource::new(&url) {
            Ok(mut source) => {
                let subscriptions: Vec<_> = EVENT_TYPES
                    .iter()
                    .filter_map(|kind| source.subscribe(*kind).ok())
                    .collect();
                let mut events = futures::stream::select_all(subscriptions);

                loop {
                    futures::select! {
                        item = events.next() => match item {
                            Some(Ok((kind, message))) => {
                                // The browser's own `open` event is a plain Event, not a
                                // MessageEvent. It has no id, and reading one throws.
                                let id = if kind == "open" {
                                    String::new()
                                } else {
                                    message.last_event_id()
                                };
                                if !id.is_empty() {
                                    *last_id.borrow_mut() = Some(id);
                                }
                                let Some(event) = parse(&kind, message.data().as_string()) else {
                                    continue;
                                };
                                if event == PushMessage::Open {
                                    failures = 0;
                                    status.set(LiveStatus::Live);
                                }
                                // Copied out first: a callback may add or remove listeners.
                                let current: Vec<_> = listeners.borrow().clone();
                                // Each callback runs as a task of its own. They set state, which
                                // re-renders components synchronously, and a panic in any of that
                                // would otherwise end this loop for good: the runtime cannot
                                // unwind, so the task is left locked and every later wake-up
                                // fails with "RefCell already borrowed". Tasks run in the order
                                // they were spawned, so events still arrive in order.
                                for (_, callback) in current {
                                    let event = event.clone();
                                    wasm_bindgen_futures::spawn_local(async move {
                                        callback.emit(event);
                                    });
                                }
                            }
                            Some(Err(_)) => {
                                status.set(LiveStatus::Paused);
                                if source.state() == State::Closed {
                                    break;
                                }
                                // Otherwise the browser is already retrying.
                            }
                            None => break,
                        },
                        _ = cancelled => {
                            source.close();
                            return;
                        }
                    }
                }
                source.close();
            }
            Err(e) => gloo::console::error!(format!("could not open the event stream: {e:?}")),
        }

        status.set(LiveStatus::Paused);
        let delay = RETRY_FIRST_MS
            .saturating_mul(2u32.saturating_pow(failures))
            .min(RETRY_MAX_MS);
        failures = failures.saturating_add(1);
        futures::select! {
            _ = gloo_timers::future::TimeoutFuture::new(delay).fuse() => {}
            _ = cancelled => return,
        }
    }
}
