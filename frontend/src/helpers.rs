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

pub fn on_change_select_element(event: Event) -> Option<web_sys::HtmlSelectElement> {
    use wasm_bindgen::JsCast;
    event
        .target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok())
}
