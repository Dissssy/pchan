use wasm_bindgen::JsValue;
use yew::prelude::*;
use yew_hooks::use_local_storage;
use yew_router::prelude::use_route;

use crate::{
    components::{
        AccountTools, AutoWatchEditor, ErrorReportEditor, InviteTools, PoweredBy, PushEditor,
        ReplyEditor, ThemeEditor, TimezoneEditor,
    },
    BaseRoute, MeContext,
};

const TAB_KEY: &str = "settings_tab";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Appearance,
    Notifications,
    Privacy,
    Invites,
    Account,
}

impl Tab {
    const ALL: [Tab; 5] = [
        Tab::Appearance,
        Tab::Notifications,
        Tab::Privacy,
        Tab::Invites,
        Tab::Account,
    ];

    /// What the address says after the `#`, so a tab can be linked to.
    fn id(self) -> &'static str {
        match self {
            Tab::Appearance => "appearance",
            Tab::Notifications => "notifications",
            Tab::Privacy => "privacy",
            Tab::Invites => "invites",
            Tab::Account => "account",
        }
    }

    fn from_id(id: &str) -> Option<Tab> {
        Tab::ALL.into_iter().find(|t| t.id() == id)
    }

    fn name(self) -> &'static str {
        match self {
            Tab::Appearance => "Appearance",
            Tab::Notifications => "Notifications",
            Tab::Privacy => "Privacy",
            Tab::Invites => "Invites",
            Tab::Account => "Account",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Tab::Appearance => "\u{f03d8}",
            Tab::Notifications => "\u{f009a}",
            Tab::Privacy => "\u{f099d}",
            Tab::Invites => "\u{f0516}",
            Tab::Account => "\u{f05d2}",
        }
    }

    /// A tab with nothing for this visitor is not shown. Signed out, the settings that belong
    /// to an account (notifications, identity) have nothing to act on.
    fn visible(self, signed_in: bool) -> bool {
        match self {
            Tab::Notifications | Tab::Account => signed_in,
            Tab::Appearance | Tab::Privacy | Tab::Invites => true,
        }
    }
}

#[function_component]
pub fn Settings() -> Html {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            document.set_title(&format!("{}Settings", crate::PREFIX));
        }
    }

    // Only the settings page owns the address bar. The popup opens over whatever page it was
    // opened on, whose own `#` may mean something (a post to scroll to).
    let route = use_route::<BaseRoute>();
    let on_page = matches!(route, Some(BaseRoute::Settings));
    let me = use_context::<MeContext>();
    let signed_in = me.is_some_and(|m| m.me.is_some());
    let emojis = use_local_storage::<bool>("emojis".to_owned());
    let glyphs = emojis.unwrap_or(true);
    let saved = use_local_storage::<String>(TAB_KEY.to_owned());

    let chosen = use_state(|| {
        let linked = on_page
            .then(|| web_sys::window().and_then(|w| w.location().hash().ok()))
            .flatten()
            .and_then(|h| Tab::from_id(h.trim_start_matches('#')));
        linked.or_else(|| (*saved).as_deref().and_then(Tab::from_id))
    });

    let tabs: Vec<Tab> = Tab::ALL
        .into_iter()
        .filter(|t| t.visible(signed_in))
        .collect();
    // What was chosen may not be offered (yet): the account arrives a moment after the page.
    let selected = (*chosen)
        .filter(|t| tabs.contains(t))
        .or_else(|| tabs.first().copied())
        .unwrap_or(Tab::Appearance);

    let pick = {
        let chosen = chosen.clone();
        let saved = saved.clone();
        move |tab: Tab| {
            let chosen = chosen.clone();
            let saved = saved.clone();
            Callback::from(move |e: MouseEvent| {
                e.prevent_default();
                chosen.set(Some(tab));
                saved.set(tab.id().to_owned());
                if on_page {
                    if let Some(history) = web_sys::window().and_then(|w| w.history().ok()) {
                        let _ = history.replace_state_with_url(
                            &JsValue::NULL,
                            "",
                            Some(&format!("#{}", tab.id())),
                        );
                    }
                }
            })
        }
    };

    html! {
        <div class="valign">
            <div class="halign settings-halign">
                <div class="settings">
                    <div class="settings-body">
                        <div class="settings-rail" role="tablist" aria-orientation="vertical">
                            { for tabs.iter().map(|tab| {
                                let active = *tab == selected;
                                html! {
                                    <button
                                        type="button"
                                        role="tab"
                                        aria-selected={active.to_string()}
                                        class={classes!("settings-tab", active.then_some("settings-tab-active"))}
                                        onclick={pick(*tab)}
                                    >
                                        if glyphs {
                                            <span class="settings-tab-icon" aria-hidden="true">{ tab.icon() }</span>
                                        }
                                        <span class="settings-tab-name">{ tab.name() }</span>
                                    </button>
                                }
                            }) }
                        </div>
                        <div class="settings-content" role="tabpanel">
                            <h2 class="settings-content-title">{ selected.name() }</h2>
                            {
                                match selected {
                                    Tab::Appearance => html! {
                                        <>
                                            <ThemeEditor />
                                            <TimezoneEditor />
                                        </>
                                    },
                                    Tab::Notifications => html! {
                                        <>
                                            <AutoWatchEditor />
                                            <PushEditor />
                                            <ReplyEditor />
                                        </>
                                    },
                                    Tab::Privacy => html! { <ErrorReportEditor /> },
                                    Tab::Invites => html! { <InviteTools expandable={false} /> },
                                    // The "copy your token" control is gone with the token it
                                    // copied: the session cookie is HttpOnly and is not
                                    // something to hand around.
                                    Tab::Account => html! { <AccountTools /> },
                                }
                            }
                        </div>
                    </div>
                    <div class="settings-footer">
                        <PoweredBy />
                    </div>
                </div>
            </div>
        </div>
    }
}
