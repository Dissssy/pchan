use yew::prelude::*;
use yew_hooks::use_local_storage;

use crate::{
    components::theme_editor::Position,
    helpers::{on_change_select_element, on_change_to_string, on_input_to_string},
    theme_data::{Color, ThemeData},
};

#[function_component]
pub fn ColorEditor(props: &Props) -> Html {
    let theme = use_context::<UseStateHandle<Option<ThemeData>>>();

    let old_color = use_state(|| None);

    let color = {
        let theme = theme.clone();
        use_state(|| match theme {
            None => ThemeData::default_dark_theme()
                .get_color(props.field.clone())
                .unwrap_or(Color::Hex("#000000".to_string())),
            Some(theme) => match theme.as_ref() {
                None => ThemeData::default_dark_theme()
                    .get_color(props.field.clone())
                    .unwrap_or(Color::Hex("#000000".to_string())),
                Some(theme) => match theme.get_color(props.field.clone()) {
                    Ok(color) => color,
                    Err(e) => {
                        gloo::console::log!(format!("Error getting color: {}", e));
                        ThemeData::default_dark_theme()
                            .get_color(props.field.clone())
                            .unwrap_or(Color::Hex("#000000".to_string()))
                    }
                },
            },
        })
    };

    let onchange = {
        let color = color.clone();
        let old_color = old_color;
        let theme = theme;
        let field = props.field.clone();
        let theme_storage = use_local_storage::<ThemeData>("theme".to_owned());
        Callback::from(move |e: Event| {
            // get the value as well as the event name of the input
            if let Some(element) = on_change_to_string(e.clone()) {
                let name = element.type_();
                if name == "color" || name == "text" {
                    // we can update the color with the current value
                    match theme {
                        None => {
                            gloo::console::log!("Theme is None");
                        }
                        Some(ref setter) => {
                            let mut current_theme = (*setter.clone())
                                .clone()
                                .unwrap_or(ThemeData::default_dark_theme());
                            if let Err(e) = current_theme.set_color(field.clone(), (*color).clone())
                            {
                                gloo::console::log!(format!("Error setting color: {}", e));
                            } else {
                                theme_storage.set(current_theme.clone());
                                setter.set(Some(current_theme));
                            }
                        }
                    }
                } else {
                    gloo::console::log!("Could not extract InputElement", e);
                }
            } else if let Some(element) = on_change_select_element(e.clone()) {
                let current_color = (*color).clone();
                match element.value().as_str() {
                    "hex" => {
                        old_color.set(Some(current_color));
                        color.set(match &*old_color {
                            Some(Color::Hex(v)) => Color::Hex(v.clone()),
                            _ => Color::Hex("#000000".to_string()),
                        });
                    }
                    "name" => {
                        old_color.set(Some(current_color));
                        color.set(match &*old_color {
                            Some(Color::Name(v)) => Color::Name(v.clone()),
                            _ => Color::Name("black".to_string()),
                        });
                    }
                    t => {
                        gloo::console::log!(format!("Unknown input type: {}", t));
                    }
                }
            } else {
                gloo::console::log!("Could not extract InputElement", e);
            }
        })
    };

    let oninput = {
        let color = color.clone();
        Callback::from(move |e: InputEvent| {
            // get the value as well as the event name of the input
            if let Some(element) = on_input_to_string(e.clone()) {
                let name = element.type_();
                let value = element.value();
                match name.as_str() {
                    "color" => {
                        color.set(Color::Hex(value));
                    }
                    "text" => {
                        color.set(Color::Name(value));
                    }
                    t => {
                        gloo::console::log!(format!("Unknown input type: {}", t));
                    }
                }
            } else {
                gloo::console::log!("Could not extract InputElement", e);
            }
        })
    };

    html! {
        <div class="color-editor" id={format!("color-editor-{}", props.position.clone().unwrap_or_default().get_id())}>
            <label for={props.field.clone()}>{props.label.clone()}</label>
            {
                match &*color {
                    Color::Hex(v) => {
                        html! {
                            <>
                                <select class="hex-or-text-dropdown" name={props.field.clone()} onchange={onchange.clone()}>
                                    <option selected=true value="hex">{"Hex Code"}</option>
                                    <option value="name">{"Color Name"}</option>
                                </select>
                                <input type="color" class="hex-color-input" value={v.clone()} oninput={oninput.clone()} onchange={onchange} />
                            </>
                        }
                    }
                    Color::Name(v) => {
                        html! {
                            <>
                                <select class="hex-or-text-dropdown" name={props.field.clone()} onchange={onchange.clone()} value="color">
                                    <option value="hex">{"Hex Code"}</option>
                                    <option selected=true value="name">{"Color Name"}</option>
                                </select>
                                <input type="text" class="text-color-input" value={v.clone()} oninput={oninput} onchange={onchange}/>
                            </>
                        }
                    }
                }
            }
            <div class="color-preview" id="ignore-transition" style={format!("background-color: {}", (*color).to_css_str())}>{"Preview"}</div>
        </div>
    }
}

#[derive(Clone, Properties, PartialEq, Debug)]
pub struct Props {
    pub label: String,
    pub field: String,
    pub position: Option<Position>,
}
