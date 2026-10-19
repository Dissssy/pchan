//! Hands an error to the reporter in `index.html`, which decides whether to send it (the
//! member may have turned reporting off) and where.

use wasm_bindgen::JsCast;

/// Report `message` as a `kind` of error. Does nothing if the reporter is not there.
///
/// This runs inside the panic hook, so it must not panic itself: a panic in a panic hook
/// aborts, and the report is lost along with the page.
pub fn send(kind: &str, message: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(reporter) = js_sys::Reflect::get(&window, &"pchanReport".into()) else {
        return;
    };
    let Ok(reporter) = reporter.dyn_into::<js_sys::Function>() else {
        return;
    };
    // The stack of the panic itself; the message only says where in the Rust it happened.
    let stack = js_sys::Reflect::get(&js_sys::Error::new(message), &"stack".into())
        .unwrap_or(wasm_bindgen::JsValue::NULL);
    let _ = reporter.call3(&window, &kind.into(), &message.into(), &stack);
}
