use yew::prelude::*;

use crate::components::{
    AccountTools, AutoWatchEditor, ErrorReportEditor, InviteTools, PoweredBy, PushEditor,
    ReplyEditor, ThemeEditor, TimezoneEditor,
};

#[function_component]
pub fn Settings() -> Html {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            document.set_title(&format!("{}Settings", crate::PREFIX));
        }
    }

    html! {
        <div class="valign">
            <div class="halign">
                <div class="settings">
                    <ThemeEditor />
                    <TimezoneEditor />
                    <AutoWatchEditor />
                    <PushEditor />
                    <ReplyEditor />
                    <ErrorReportEditor />
                    <InviteTools expandable={true} />
                    // The "copy your token" control is gone with the token it copied: the
                    // session cookie is HttpOnly and is not something to hand around.
                    <AccountTools />
                    <PoweredBy />
                </div>
            </div>
        </div>
    }
}
