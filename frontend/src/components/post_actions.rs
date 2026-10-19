//! What a viewer can do to a post, from its header.
//!
//! The header shows one cell. With nothing to do it is empty, with one action it is that
//! action's icon, and with several it is a gavel that opens a menu. Every action that needs a
//! decision goes through one modal, which stays open when the server refuses and closes (with
//! a toast) when it does not.
//!
//! Only what the viewer can actually do is offered. The server checks all of it again; this is
//! about not showing buttons that can only fail.

use common::structs::{MeInfo, SafePost, User};
use yew::prelude::*;

use crate::{authored::use_authored, helpers::preview, ApiContext, MeContext};

const ICON_DELETE: &str = "\u{ea81}";
const ICON_REPORT: &str = "\u{f024}";
const ICON_BAN: &str = "\u{f05e}";
const ICON_MENU: &str = "\u{f0e3}";
const ICON_LOADING: &str = "\u{f1ce}";

const TOAST_MS: u32 = 4_000;
const PREVIEW_CHARS: usize = 400;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PostAction {
    Delete,
    Report,
    BanBoard,
    BanSite,
}

impl PostAction {
    fn icon(self) -> &'static str {
        match self {
            Self::Delete => ICON_DELETE,
            Self::Report => ICON_REPORT,
            Self::BanBoard | Self::BanSite => ICON_BAN,
        }
    }

    /// Names the action in a menu, as a tooltip, and as the modal's title.
    fn label(self, board: &str) -> String {
        match self {
            Self::Delete => "Delete this post".to_owned(),
            Self::Report => "Report this post".to_owned(),
            Self::BanBoard => format!("Ban this post's author from /{board}/"),
            Self::BanSite => "Ban this post's author from the whole site".to_owned(),
        }
    }

    fn confirm_label(self) -> &'static str {
        match self {
            Self::Delete => "Delete post",
            Self::Report => "Send report",
            Self::BanBoard => "Ban from board",
            Self::BanSite => "Ban from site",
        }
    }

    fn class(self) -> &'static str {
        match self {
            Self::Delete => "post-action-delete",
            Self::Report => "post-action-report",
            Self::BanBoard => "post-action-ban",
            Self::BanSite => "post-action-ban post-action-sitewide",
        }
    }

    /// Whether the modal asks for words, and whether they are required.
    fn reason(self) -> Option<bool> {
        match self {
            Self::Delete => None,
            Self::Report => Some(true),
            Self::BanBoard | Self::BanSite => Some(false),
        }
    }

    /// Say what the action does, below the preview.
    fn warning(self, board: &str) -> Option<String> {
        match self {
            Self::Delete => Some("This removes the post for everyone.".to_owned()),
            Self::Report => None,
            Self::BanBoard => Some(format!(
                "You will not be told who this is. They are banned from /{board}/ and the ban is logged."
            )),
            Self::BanSite => Some(
                "You will not be told who this is. They are banned from every board on the site."
                    .to_owned(),
            ),
        }
    }
}

/// The actions this viewer has on a post, in the order they are listed.
pub fn actions_for(me: &MeInfo, board: &str, mine: bool) -> Vec<PostAction> {
    // Nothing but deleting is useful on a post that is yours: reporting or banning yourself
    // is meaningless.
    if mine {
        return vec![PostAction::Delete];
    }
    let mut actions = Vec::new();
    let board_mod = me.can_moderate_board(board);
    if board_mod {
        actions.push(PostAction::Delete);
    }
    actions.push(PostAction::Report);
    if board_mod {
        actions.push(PostAction::BanBoard);
    }
    if me.can_moderate_site() {
        actions.push(PostAction::BanSite);
    }
    actions
}

// ---- the header cell --------------------------------------------------------------------

/// A faint turning glyph for a header cell whose contents are not known yet. It takes the
/// space the real icon will, so nothing shifts when the answer arrives.
#[derive(Properties, PartialEq)]
pub struct PlaceholderProps {
    #[prop_or_default]
    pub class: Classes,
}

#[function_component]
pub fn HeaderPlaceholder(props: &PlaceholderProps) -> Html {
    html! {
        <div class={classes!("post-action-cell", props.class.clone())}>
            <span class="header-spinner" title="Loading" aria-busy="true">{ ICON_LOADING }</span>
        </div>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct Props {
    pub post: SafePost,
}

#[function_component]
pub fn PostActions(props: &Props) -> Html {
    let me = use_context::<MeContext>().and_then(|m| m.me);
    let ctx = use_context::<PostActionsContext>();
    let authored = use_authored();
    let menu_at = use_state(|| None::<(f64, f64)>);
    let menu_ref = use_node_ref();
    let button_ref = use_node_ref();

    {
        let menu_ref = menu_ref.clone();
        use_effect_with(menu_at.is_some(), move |open| {
            if *open {
                if let Some(el) = menu_ref.cast::<web_sys::HtmlElement>() {
                    let _ = el.focus();
                }
            }
        });
    }

    // Signed out: there is nothing to offer, and nothing to wait for either.
    let (Some(me), Some(ctx)) = (me, ctx) else {
        return html! {};
    };

    let board = props.post.board_discriminator.as_str();
    let Some(mine) = authored.is_mine(board, props.post.post_number) else {
        return html! { <HeaderPlaceholder /> };
    };

    let actions = actions_for(&me, board, mine);
    let open_action = {
        let post = props.post.clone();
        let open = ctx.open.clone();
        move |action: PostAction| {
            let post = post.clone();
            let open = open.clone();
            Callback::from(move |e: MouseEvent| {
                e.prevent_default();
                open.emit((action, post.clone()));
            })
        }
    };

    match actions.as_slice() {
        [] => html! {},
        [only] => html! {
            <div class="post-action-cell">
                <button
                    type="button"
                    class={classes!("post-action-button", only.class())}
                    title={only.label(board)}
                    aria-label={only.label(board)}
                    onclick={open_action(*only)}
                >{ only.icon() }</button>
            </div>
        },
        many => {
            let toggle = {
                let menu_at = menu_at.clone();
                let button_ref = button_ref.clone();
                Callback::from(move |e: MouseEvent| {
                    e.prevent_default();
                    if menu_at.is_some() {
                        menu_at.set(None);
                        return;
                    }
                    // Fixed to the viewport, so a header that clips its overflow cannot clip
                    // the menu. Measured through a ref: Yew listens once at the page root, so
                    // the event's current target is the root, not this button.
                    let at = button_ref
                        .cast::<web_sys::Element>()
                        .map(|el| {
                            let r = el.get_bounding_client_rect();
                            (r.left(), r.bottom())
                        })
                        .unwrap_or((0.0, 0.0));
                    menu_at.set(Some(at));
                })
            };
            let close = {
                let menu_at = menu_at.clone();
                Callback::from(move |_: MouseEvent| menu_at.set(None))
            };
            let on_key = {
                let menu_at = menu_at.clone();
                Callback::from(move |e: KeyboardEvent| {
                    if e.key() == "Escape" {
                        menu_at.set(None);
                    }
                })
            };
            html! {
                <div class="post-action-cell">
                    <button
                        type="button"
                        class="post-action-button post-action-menu-button"
                        ref={button_ref}
                        title="Moderation"
                        aria-label="Moderation"
                        aria-haspopup="menu"
                        aria-expanded={menu_at.is_some().to_string()}
                        onclick={toggle}
                    >{ ICON_MENU }</button>
                    if let Some((x, y)) = *menu_at {
                        <div class="post-action-backdrop" onclick={close.clone()} />
                        <div
                            class="post-action-menu"
                            role="menu"
                            tabindex="-1"
                            ref={menu_ref}
                            style={format!("left: {x}px; top: {y}px;")}
                            onkeydown={on_key}
                        >
                            { for many.iter().map(|action| {
                                let open = open_action(*action);
                                let close = close.clone();
                                html! {
                                    <button
                                        type="button"
                                        role="menuitem"
                                        class={classes!("post-action-item", action.class())}
                                        onclick={Callback::from(move |e: MouseEvent| {
                                            close.emit(e.clone());
                                            open.emit(e);
                                        })}
                                    >
                                        <span class="post-action-item-icon">{ action.icon() }</span>
                                        { action.label(board) }
                                    </button>
                                }
                            }) }
                        </div>
                    }
                </div>
            }
        }
    }
}

// ---- the modal and the toasts ------------------------------------------------------------

/// Reaches the one modal that lives at the top of the page.
#[derive(Clone, PartialEq)]
pub struct PostActionsContext {
    pub open: Callback<(PostAction, SafePost)>,
}

#[derive(Clone, PartialEq)]
struct Pending {
    action: PostAction,
    post: SafePost,
}

#[derive(Default, PartialEq, Clone)]
struct Toasts(Vec<(u64, String)>);

enum ToastAction {
    Add(u64, String),
    Remove(u64),
}

impl Reducible for Toasts {
    type Action = ToastAction;

    fn reduce(self: std::rc::Rc<Self>, action: ToastAction) -> std::rc::Rc<Self> {
        let mut next = self.0.clone();
        match action {
            ToastAction::Add(id, text) => next.push((id, text)),
            ToastAction::Remove(id) => next.retain(|(i, _)| *i != id),
        }
        std::rc::Rc::new(Toasts(next))
    }
}

#[derive(Properties, PartialEq)]
pub struct ProviderProps {
    pub children: Html,
}

#[function_component]
pub fn PostActionsProvider(props: &ProviderProps) -> Html {
    let pending = use_state(|| None::<Pending>);
    let toasts = use_reducer(Toasts::default);
    let next_id = use_mut_ref(|| 0u64);

    let open = {
        let pending = pending.clone();
        Callback::from(move |(action, post): (PostAction, SafePost)| {
            pending.set(Some(Pending { action, post }));
        })
    };
    let close = {
        let pending = pending.clone();
        Callback::from(move |_: ()| pending.set(None))
    };
    let done = {
        let pending = pending.clone();
        let dispatch = toasts.dispatcher();
        Callback::from(move |message: String| {
            pending.set(None);
            let mut next = next_id.borrow_mut();
            *next += 1;
            dispatch.dispatch(ToastAction::Add(*next, message));
        })
    };
    let remove = {
        let dispatch = toasts.dispatcher();
        Callback::from(move |id: u64| dispatch.dispatch(ToastAction::Remove(id)))
    };

    html! {
        <ContextProvider<PostActionsContext> context={PostActionsContext { open }}>
            { props.children.clone() }
            if let Some(p) = &*pending {
                <ActionModal
                    key={format!("{:?}/{}/{}", p.action, p.post.board_discriminator, p.post.post_number)}
                    action={p.action}
                    post={p.post.clone()}
                    on_close={close}
                    on_done={done}
                />
            }
            <div class="post-action-toasts">
                { for toasts.0.iter().map(|(id, text)| html! {
                    <ActionToast key={*id} id={*id} text={text.clone()} remove={remove.clone()} />
                }) }
            </div>
        </ContextProvider<PostActionsContext>>
    }
}

#[derive(Properties, PartialEq)]
struct ToastProps {
    id: u64,
    text: String,
    remove: Callback<u64>,
}

#[function_component]
fn ActionToast(props: &ToastProps) -> Html {
    {
        let remove = props.remove.clone();
        let id = props.id;
        use_effect_with((), move |_| {
            let timer = gloo_timers::callback::Timeout::new(TOAST_MS, move || remove.emit(id));
            move || drop(timer)
        });
    }
    let dismiss = {
        let remove = props.remove.clone();
        let id = props.id;
        Callback::from(move |_: MouseEvent| remove.emit(id))
    };
    html! {
        <div class="post-action-toast" role="status" onclick={dismiss}>{ props.text.clone() }</div>
    }
}

#[derive(Properties, PartialEq)]
struct ModalProps {
    action: PostAction,
    post: SafePost,
    on_close: Callback<()>,
    on_done: Callback<String>,
}

#[function_component]
fn ActionModal(props: &ModalProps) -> Html {
    let api_ctx = use_context::<Option<ApiContext>>().flatten();
    let reason = use_state(String::new);
    let busy = use_state(|| false);
    let error = use_state(|| None::<String>);
    let focus_ref = use_node_ref();

    {
        let focus_ref = focus_ref.clone();
        use_effect_with((), move |_| {
            if let Some(el) = focus_ref.cast::<web_sys::HtmlElement>() {
                let _ = el.focus();
            }
        });
    }

    let action = props.action;
    let board = props.post.board_discriminator.clone();
    let number = props.post.post_number;

    let submit = {
        let reason = reason.clone();
        let busy = busy.clone();
        let error = error.clone();
        let on_done = props.on_done.clone();
        let board = board.clone();
        Callback::from(move |_: ()| {
            if *busy {
                return;
            }
            let words = reason.trim().to_owned();
            if action.reason() == Some(true) && words.is_empty() {
                error.set(Some("Say what is wrong with it.".to_owned()));
                return;
            }
            let Some(api) = api_ctx.as_ref().and_then(|c| c.api.as_ref().ok()).cloned() else {
                error.set(Some("No API context".to_owned()));
                return;
            };
            busy.set(true);
            error.set(None);
            let busy = busy.clone();
            let error = error.clone();
            let on_done = on_done.clone();
            let board = board.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let optional = (!words.is_empty()).then(|| words.clone());
                let res = match action {
                    PostAction::Delete => api
                        .delete_post(board.as_str(), number)
                        .await
                        .map(|_| "Post deleted".to_owned()),
                    PostAction::Report => api.report_post(&board, number, words).await,
                    PostAction::BanBoard => api.ban_post_author(&board, number, false, optional).await,
                    PostAction::BanSite => api.ban_post_author(&board, number, true, optional).await,
                };
                busy.set(false);
                match res {
                    Ok(message) => on_done.emit(message),
                    Err(e) => error.set(Some(format!("{}", *e))),
                }
            });
        })
    };

    let on_submit = {
        let submit = submit.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            submit.emit(());
        })
    };
    let cancel = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| on_close.emit(()))
    };
    let on_key = {
        let on_close = props.on_close.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Escape" {
                on_close.emit(());
            }
        })
    };
    let on_input = {
        let reason = reason.clone();
        Callback::from(move |e: InputEvent| {
            let area: web_sys::HtmlTextAreaElement = e.target_unchecked_into();
            reason.set(area.value());
        })
    };

    let author = match &props.post.author {
        User::Anonymous => "Anonymous".to_owned(),
        User::Named(n) => n.clone(),
        User::Mod(n) => format!("{n} ## MOD"),
    };
    let body = preview(&props.post.content, PREVIEW_CHARS);

    html! {
        <div class="post-action-overlay" onkeydown={on_key}>
            <div class="post-action-overlay-backdrop" onclick={cancel.clone()} />
            <form
                class={classes!("post-action-modal", action.class())}
                role="dialog"
                aria-modal="true"
                onsubmit={on_submit}
            >
                <h2 class="post-action-title">{ action.label(&board) }</h2>
                <div class="post-action-preview">
                    <div class="post-action-preview-head">
                        <span>{ author }</span>
                        <span class="post-action-preview-number">{ format!("#{number}") }</span>
                    </div>
                    if props.post.file.is_some() {
                        <div class="post-action-preview-file">{ "(has an attached file)" }</div>
                    }
                    if !body.is_empty() {
                        <div class="post-action-preview-body">{ body }</div>
                    }
                </div>
                if let Some(warning) = action.warning(&board) {
                    <p class="post-action-warning">{ warning }</p>
                }
                if let Some(required) = action.reason() {
                    <textarea
                        class="post-action-reason"
                        ref={focus_ref.clone()}
                        rows="3"
                        maxlength="1000"
                        placeholder={ if required { "What is wrong with this post?" } else { "Reason (optional)" } }
                        value={(*reason).clone()}
                        oninput={on_input}
                        disabled={*busy}
                    />
                }
                if let Some(e) = &*error {
                    <p class="post-action-error error" role="alert">{ e.clone() }</p>
                }
                <div class="post-action-buttons">
                    // With no text field to focus, the safe choice gets it.
                    <button
                        type="button"
                        class="post-action-cancel"
                        ref={ if action.reason().is_none() { focus_ref } else { NodeRef::default() } }
                        onclick={cancel}
                    >{ "Cancel" }</button>
                    <button type="submit" class="post-action-confirm" disabled={*busy}>
                        { if *busy { "…" } else { action.confirm_label() } }
                    </button>
                </div>
            </form>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{actions_for, PostAction};
    use common::structs::{MeInfo, ModScope};

    fn me(moderates: ModScope) -> MeInfo {
        MeInfo {
            role: "member".into(),
            banned: false,
            moderates,
            cache_key: String::new(),
            reply_push: false,
        }
    }

    #[test]
    fn a_member_reports_others_and_deletes_their_own() {
        let m = me(ModScope::Boards(vec![]));
        assert_eq!(actions_for(&m, "b", false), vec![PostAction::Report]);
        assert_eq!(actions_for(&m, "b", true), vec![PostAction::Delete]);
    }

    #[test]
    fn a_board_mod_is_scoped_to_their_board() {
        let m = me(ModScope::Boards(vec!["b".into()]));
        assert_eq!(
            actions_for(&m, "b", false),
            vec![PostAction::Delete, PostAction::Report, PostAction::BanBoard]
        );
        assert_eq!(actions_for(&m, "a", false), vec![PostAction::Report]);
        assert_eq!(actions_for(&m, "b", true), vec![PostAction::Delete]);
    }

    #[test]
    fn site_scope_adds_the_sitewide_ban_everywhere() {
        let m = me(ModScope::site());
        assert_eq!(
            actions_for(&m, "x", false),
            vec![
                PostAction::Delete,
                PostAction::Report,
                PostAction::BanBoard,
                PostAction::BanSite
            ]
        );
        assert_eq!(actions_for(&m, "x", true), vec![PostAction::Delete]);
    }
}
