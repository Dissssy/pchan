use common::structs::{ThreadNotification, User};
use yew::prelude::*;
use yew_hooks::use_effect_once;
use yew_router::prelude::{use_navigator, use_route};

use crate::{
    components::richtext::SpoilableText,
    events::{use_live_status, LiveStatus},
    helpers::{format_short_timestamp, preview, reply_targets},
    inbox::{use_inbox, InboxContext, ThreadKey},
    BaseRoute,
};

/// How long a toast stays before it fades on its own.
const TOAST_MS: u32 = 8_000;
const PREVIEW_CHARS: usize = 160;

/// The bell, its inbox panel, and the toasts. Nothing is drawn for a signed-out visitor.
#[function_component]
pub fn NotificationBox() -> Html {
    let inbox = use_inbox();
    let live = use_live_status();
    let open = use_state(|| false);

    let paused = live == LiveStatus::Paused;
    let Some(inbox) = inbox else {
        return html! {};
    };
    let signed_in = inbox.state.loaded();

    let toggle = {
        let open = open.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            open.set(!*open);
        })
    };
    let close = {
        let open = open.clone();
        Callback::from(move |_: MouseEvent| open.set(false))
    };

    let total = inbox.state.unread_total();
    html! {
        <>
            if paused {
                // Fades in after a few seconds, so a reconnect that takes a moment is not
                // announced.
                <div class="live-paused">{ "Live updates paused, reconnecting…" }</div>
            }
            if signed_in {
                <div class="notification-bell" onclick={toggle} title="Notifications">
                    <span class="notification-bell-icon">{ "\u{f009a}" }</span>
                    if total > 0 {
                        <span class="notification-badge">{ if total > 99 { "99+".to_owned() } else { total.to_string() } }</span>
                    }
                </div>
                if *open {
                    <div class="notification-backdrop" onclick={close.clone()} />
                    <NotificationPanel inbox={inbox.clone()} on_close={close} />
                }
                <div class="notification-toasts">
                    { for inbox.state.toasts.iter().map(|toast| html! {
                        <ToastCard key={toast.id} id={toast.id} item={toast.item.clone()} reply={toast.reply} inbox={inbox.clone()} />
                    }) }
                </div>
            }
        </>
    }
}

#[derive(Properties, PartialEq)]
struct PanelProps {
    inbox: InboxContext,
    on_close: Callback<MouseEvent>,
}

#[function_component]
fn NotificationPanel(props: &PanelProps) -> Html {
    let inbox = &props.inbox;
    let items = &inbox.state.items;
    let mark_all = {
        let cb = inbox.mark_all_read.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };

    let replies = &inbox.state.replies;
    html! {
        <div class="notification-panel">
            <div class="notification-panel-header">
                <span class="notification-panel-title">{ "Notifications" }</span>
                if !items.is_empty() {
                    <span class="notification-action" onclick={mark_all}>{ "mark all read" }</span>
                }
            </div>
            if items.is_empty() && replies.is_empty() {
                <div class="notification-empty">{ "Nothing unread." }</div>
            }
            if !replies.is_empty() {
                <div class="notification-section">{ "Replies to you" }</div>
            }
            { for replies.iter().map(|item| html! {
                <NotificationCard
                    key={format!("reply/{}/{}", item.board_discriminator, item.latest.post_number)}
                    item={item.clone()}
                    inbox={inbox.clone()}
                    toast={None::<u64>}
                    reply={true}
                    on_navigate={props.on_close.clone()}
                />
            }) }
            if !replies.is_empty() && !items.is_empty() {
                <div class="notification-section">{ "Watched threads" }</div>
            }
            { for items.iter().map(|item| html! {
                <NotificationCard
                    key={format!("{}/{}", item.board_discriminator, item.thread_post_number)}
                    item={item.clone()}
                    inbox={inbox.clone()}
                    toast={None::<u64>}
                    reply={false}
                    on_navigate={props.on_close.clone()}
                />
            }) }
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct ToastProps {
    id: u64,
    item: ThreadNotification,
    reply: bool,
    inbox: InboxContext,
}

/// A notification that appears for a few seconds and goes away by itself.
#[function_component]
fn ToastCard(props: &ToastProps) -> Html {
    {
        let dismiss = props.inbox.dismiss_toast.clone();
        let id = props.id;
        use_effect_once(move || {
            let timer = gloo_timers::callback::Timeout::new(TOAST_MS, move || dismiss.emit(id));
            move || drop(timer)
        });
    }
    html! {
        <NotificationCard
            item={props.item.clone()}
            inbox={props.inbox.clone()}
            toast={Some(props.id)}
            reply={props.reply}
            on_navigate={Callback::noop()}
        />
    }
}

#[derive(Properties, PartialEq)]
struct CardProps {
    item: ThreadNotification,
    inbox: InboxContext,
    /// `Some(id)` when shown as a toast, which can be dismissed rather than marked read.
    toast: Option<u64>,
    /// A reply to one of the member's own posts, rather than news in a watched thread.
    reply: bool,
    on_navigate: Callback<MouseEvent>,
}

/// One watched thread with unread posts, drawn like a post: the same header bar and body, so it
/// reads as part of the site rather than a system dialog.
#[function_component]
fn NotificationCard(props: &CardProps) -> Html {
    let item = &props.item;
    let latest = &item.latest;
    let timezone = use_context::<UseStateHandle<chrono_tz::Tz>>();
    let navigator = use_navigator();
    let route = use_route::<BaseRoute>();

    let key: ThreadKey = (item.board_discriminator.clone(), item.thread_post_number);
    let go = {
        let (board, thread, first) = (
            item.board_discriminator.clone(),
            item.thread_post_number,
            item.first_unread,
        );
        let on_navigate = props.on_navigate.clone();
        let dismiss = props.toast.map(|id| (id, props.inbox.dismiss_toast.clone()));
        let reply_key = props
            .reply
            .then(|| (item.board_discriminator.clone(), item.latest.post_number));
        let dismiss_reply = props.inbox.dismiss_reply.clone();
        let already_here = route
            == Some(BaseRoute::ThreadPage {
                board_discriminator: board.clone(),
                thread_id: thread.to_string(),
            });
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            on_navigate.emit(e);
            if let Some((id, dismiss)) = &dismiss {
                dismiss.emit(*id);
            }
            // Opening a reply is dealing with it.
            if let Some(key) = &reply_key {
                dismiss_reply.emit(key.clone());
            }
            if !already_here {
                if let Some(nav) = navigator.as_ref() {
                    nav.push(&BaseRoute::ThreadPage {
                        board_discriminator: board.clone(),
                        thread_id: thread.to_string(),
                    });
                }
            }
            jump_to_post(first);
        })
    };
    // Buttons sit inside the clickable card; they must not also open the thread.
    let button = |on_click: Callback<()>| {
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            e.stop_propagation();
            on_click.emit(());
        })
    };
    let mark_read = {
        let cb = props.inbox.mark_read.clone();
        let dismiss_reply = props.inbox.dismiss_reply.clone();
        let is_reply = props.reply;
        let (board, thread, through) = (key.0.clone(), key.1, latest.post_number);
        button(Callback::from(move |_| {
            // A reply lives only in this browser's list; a watched thread has a read mark on
            // the server.
            if is_reply {
                dismiss_reply.emit((board.clone(), through));
            } else {
                cb.emit((board.clone(), thread, through));
            }
        }))
    };
    let unwatch = {
        let cb = props.inbox.stop_watching.clone();
        let key = key.clone();
        button(Callback::from(move |_| cb.emit(key.clone())))
    };
    // Dismissing a toast means "I've dealt with this": mark the thread read, so it leaves the
    // inbox as well, here and on every other tab and device.
    let dismiss = props.toast.map(|id| {
        let dismiss_toast = props.inbox.dismiss_toast.clone();
        let mark_read = props.inbox.mark_read.clone();
        let dismiss_reply = props.inbox.dismiss_reply.clone();
        let is_reply = props.reply;
        let (board, thread, through) = (key.0.clone(), key.1, latest.post_number);
        button(Callback::from(move |_| {
            if is_reply {
                dismiss_reply.emit((board.clone(), through));
            } else {
                mark_read.emit((board.clone(), thread, through));
            }
            dismiss_toast.emit(id);
        }))
    });

    let author = match &latest.author {
        User::Anonymous => "Anonymous".to_owned(),
        User::Named(name) => name.clone(),
        User::Mod(name) => format!("{name} ## MOD"),
    };
    let count = if item.unread == 1 {
        "1 new".to_owned()
    } else {
        format!("{} new", item.unread)
    };

    // Does this reply to one of the member's own posts? Known from what this browser
    // remembers; anything it does not know yet is asked about, and this redraws when it does.
    let authored = crate::authored::use_authored();
    let targets = reply_targets(&latest.content, &item.board_discriminator, item.thread_post_number);
    let replies_to_me = props.reply
        || targets
            .iter()
            .any(|(board, number)| authored.is_mine(board, *number) == Some(true));
    {
        let unknown: Vec<(String, i64)> = targets
            .iter()
            .filter(|(board, number)| authored.is_mine(board, *number).is_none())
            .cloned()
            .collect();
        let rev = authored.rev;
        use_effect_with((unknown.clone(), rev), move |(unknown, _)| {
            if !unknown.is_empty() {
                authored.want(unknown.clone());
            }
        });
    }

    html! {
        <div class={classes!("post", "notification-card", props.toast.map(|_| "notification-toast"), replies_to_me.then_some("notification-reply"))} onclick={go}>
            <div class="post-header">
                if let Some(dismiss) = dismiss {
                    <span class="notification-icon-button" title="Dismiss, and mark read" onclick={dismiss}>{ "\u{f0156}" }</span>
                }
                <span class="post-topic">{ &item.topic }</span>
                if !props.reply {
                    <span class="post-number">{ count }</span>
                }
                if replies_to_me {
                    <span class="notification-reply-chip" title="Replies to one of your posts">{ "\u{f0450} replied to you" }</span>
                }
                <span class="post-author">{ author }</span>
                <span class="post-timestamp">{ format_short_timestamp(&latest.timestamp, timezone.as_deref()) }</span>
                if props.toast.is_none() {
                    <span class="notification-actions">
                        <span class="notification-icon-button" title="Mark read" onclick={mark_read}>{ "\u{f012c}" }</span>
                        if !props.reply {
                            <span class="notification-icon-button" title="Stop watching this thread" onclick={unwatch}>{ "\u{f0209}" }</span>
                        }
                    </span>
                }
            </div>
            <div class="post-body">
                if let Some(file) = latest.file.as_ref() {
                    <img class="notification-thumb" src={file.claimed.thumbnail.clone()} />
                }
                <div class="post-content">
                    <SpoilableText content={preview(&latest.content, PREVIEW_CHARS)} />
                </div>
            </div>
        </div>
    }
}

/// Scroll to a post in the open thread, with the page's own highlight. The thread may still be
/// loading, so this waits a moment.
fn jump_to_post(post_number: i64) {
    gloo_timers::callback::Timeout::new(400, move || {
        let Some(window) = web_sys::window() else {
            return;
        };
        if let Ok(history) = window.history() {
            let path = window
                .location()
                .pathname()
                .unwrap_or_else(|_| "/".to_owned());
            let _ = history.replace_state_with_url(
                &wasm_bindgen::JsValue::NULL,
                "",
                Some(&format!("{path}#{post_number}")),
            );
        }
        if let Ok(f) = js_sys::Reflect::get(&window, &"scroll_to_post".into()) {
            if let Ok(f) = wasm_bindgen::JsCast::dyn_into::<js_sys::Function>(f) {
                let _ = f.call0(&window);
            }
        }
    })
    .forget();
}
