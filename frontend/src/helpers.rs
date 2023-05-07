use common::structs::{Reply, SafePost};
use web_sys::HtmlInputElement;
use yew::prelude::*;

pub fn on_input_to_string(event: InputEvent) -> Option<HtmlInputElement> {
    use wasm_bindgen::JsCast;
    event
        .target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
}

pub fn on_change_to_string(event: Event) -> Option<HtmlInputElement> {
    use wasm_bindgen::JsCast;
    event
        .target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
}

pub fn on_change_textarea_to_string(event: Event) -> Option<web_sys::HtmlTextAreaElement> {
    use wasm_bindgen::JsCast;
    event
        .target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
}

pub fn on_input_textarea_to_string(event: InputEvent) -> Option<web_sys::HtmlTextAreaElement> {
    use wasm_bindgen::JsCast;
    event
        .target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
}

pub fn on_change_select_element(event: Event) -> Option<web_sys::HtmlSelectElement> {
    use wasm_bindgen::JsCast;
    event
        .target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok())
}

#[derive(Clone, PartialEq, Debug)]
pub struct CallbackEmitterContext {
    pub callback: Callback<Callback<Reply>>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct CallbackContext {
    pub callback: Callback<Reply>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct SuccessfulPostContext {
    pub callback: Callback<SafePost>,
}
