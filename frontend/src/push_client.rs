//! Push notifications on this device, as far as the page is concerned. The work is done by the
//! functions in `index.html`; this is a way to call them from Rust.

use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PushState {
    /// Subscribed on this device.
    On,
    /// Allowed, or not yet asked, but not subscribed.
    Off,
    /// The member refused permission in the browser; only the browser's own settings can change it.
    Denied,
    /// This browser cannot do push at all.
    Unsupported,
    /// Something failed (the server or the push service said no).
    Error,
}

impl PushState {
    fn from_name(name: &str) -> Self {
        match name {
            "on" => Self::On,
            "denied" => Self::Denied,
            "unsupported" => Self::Unsupported,
            "error" => Self::Error,
            _ => Self::Off,
        }
    }
}

/// Call a function the page defines, if it is there, and wait for the string it resolves to.
async fn call(name: &str) -> Option<String> {
    let window = web_sys::window()?;
    let function = js_sys::Reflect::get(&window, &JsValue::from_str(name))
        .ok()?
        .dyn_into::<js_sys::Function>()
        .ok()?;
    let promise = function.call0(&window).ok()?.dyn_into::<js_sys::Promise>().ok()?;
    JsFuture::from(promise).await.ok()?.as_string()
}

/// Whether this device is subscribed.
pub async fn state() -> PushState {
    call("pchanPushState")
        .await
        .map_or(PushState::Unsupported, |s| PushState::from_name(&s))
}

/// Ask for permission if needed and subscribe. Only call this from a click.
pub async fn enable() -> PushState {
    call("pchanEnablePush")
        .await
        .map_or(PushState::Error, |s| PushState::from_name(&s))
}

pub async fn disable() -> PushState {
    call("pchanDisablePush")
        .await
        .map_or(PushState::Error, |s| PushState::from_name(&s))
}

/// Re-tell the server about a subscription this browser already has, for whoever is signed in.
pub async fn sync() {
    let _ = call("pchanSyncPush").await;
}
