use yew::prelude::*;
use yew_hooks::UseLocalStorageHandle;

use crate::{
    components::theme_editor::Position,
    helpers::{on_change_to_string, on_input_to_string},
};

#[function_component]
pub fn ColorEditor(props: &Props) -> Html {
    let onchange = {
        let field = props.field.clone();
        Callback::from(move |e: Event| {
            // get the value as well as the event name of the input
            if let Some(element) = on_change_to_string(e.clone()) {
                field.set(element.value());
            } else {
                gloo::console::log!("Could not extract InputElement", e);
            }
        })
    };

    let oninput = {
        let field = props.field.clone();
        Callback::from(move |e: InputEvent| {
            // get the value as well as the event name of the input
            if let Some(element) = on_input_to_string(e.clone()) {
                let value = element.value();
                field.set(value);
            } else {
                gloo::console::log!("Could not extract InputElement", e);
            }
        })
    };

    html! {
        <div class="color-editor" id={format!("color-editor-{}", props.position.clone().unwrap_or_default().get_id())}>
            <label for="color">{props.label.clone()}</label>
            <input type="color" class="hex-color-input" value={(*props.field).clone().unwrap_or_default()} oninput={oninput.clone()} onchange={onchange} />
            <div class="color-preview" id="ignore-transition" style={format!("background-color: {}", (*props.field).clone().unwrap_or_default())}>{"Preview"}</div>
        </div>
    }
}

#[derive(Clone, Properties, PartialEq)]
pub struct Props {
    pub label: AttrValue,
    pub field: UseLocalStorageHandle<String>,
    pub position: Option<Position>,
}
