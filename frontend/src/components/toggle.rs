use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew_hooks::use_local_storage;

const BOX_ON: &str = "\u{f14a}";
const BOX_OFF: &str = "\u{f096}";
const SPINNER: &str = "\u{f1ce}";
const WARNING: &str = "\u{f071}";

/// A setting that is on or off: a label, a checkbox drawn as a glyph, and a note underneath.
///
/// The checkbox is still a real one, hidden under the glyph, so keyboards and screen readers
/// treat it as one. `onchange` is handed the value the member asked for; the toggle keeps
/// showing `checked` until the owner changes it, so a change that fails or is refused simply
/// leaves it where it was.
#[derive(Properties, PartialEq)]
pub struct Props {
    pub checked: bool,
    pub onchange: Callback<bool>,
    pub label: AttrValue,
    #[prop_or_default]
    pub disabled: bool,
    /// Waiting on something slow; shows a turning glyph and ignores clicks.
    #[prop_or_default]
    pub pending: bool,
    /// The note says something the member should read before turning this on.
    #[prop_or_default]
    pub warn: bool,
    /// The note under the label.
    #[prop_or_default]
    pub children: Html,
}

#[function_component]
pub fn Toggle(props: &Props) -> Html {
    // The same switch the rest of the site honours: glyphs, or words where they are off.
    let emojis = use_local_storage::<bool>("emojis".to_owned());
    let glyphs = emojis.unwrap_or(true);

    let blocked = props.disabled || props.pending;
    let onchange = {
        let checked = props.checked;
        let emit = props.onchange.clone();
        Callback::from(move |e: Event| {
            if blocked {
                return;
            }
            emit.emit(!checked);
            // Whatever the browser just did to the box, it shows what the owner says.
            if let Some(input) = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
            {
                input.set_checked(checked);
            }
        })
    };

    let mark = if props.pending {
        if glyphs { SPINNER } else { "[…]" }
    } else if props.checked {
        if glyphs { BOX_ON } else { "[x]" }
    } else if glyphs {
        BOX_OFF
    } else {
        "[ ]"
    };

    html! {
        <div class={classes!("setting-toggle", blocked.then_some("setting-toggle-blocked"))}>
            <label class="setting-toggle-row">
                <input
                    type="checkbox"
                    class="setting-toggle-input"
                    checked={props.checked}
                    disabled={blocked}
                    {onchange}
                />
                <span class={classes!("setting-toggle-box", (props.pending && glyphs).then_some("header-spinner"))} aria-hidden="true">
                    { mark }
                </span>
                <span class="setting-toggle-label">{ props.label.clone() }</span>
            </label>
            <div class={classes!("setting-toggle-note", props.warn.then_some("setting-toggle-note-warn"))}>
                if props.warn {
                    <span class="setting-toggle-warning">{ if glyphs { WARNING } else { "Note: " } }</span>
                }
                { props.children.clone() }
            </div>
        </div>
    }
}
