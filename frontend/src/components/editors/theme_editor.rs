// use gloo_timers::callback;
use yew::prelude::*;
use yew_hooks::prelude::*;

use crate::ThemeData;

use super::color_editor::ColorEditor;

#[function_component]
pub fn ThemeEditor() -> Html {
    let emojis = use_local_storage::<bool>("emojis".to_owned());

    let theme = use_context::<ThemeData>();

    let emoji_cycle = use_state(|| {
        if emojis.unwrap_or(true) {
            EmojiState::Enabled
        } else {
            EmojiState::Disabled
        }
    });

    // let refresh_val = use_state(|| true);

    // let proc_refresh = {
    //     let refresh_val = refresh_val.clone();
    //     Callback::from(move |()| {
    //         refresh_val.set(false);
    //         // wait for 0.1 seconds
    //         let refresh_val = refresh_val.clone();
    //         callback::Timeout::new(10, move || {
    //             refresh_val.set(true);
    //         })
    //         .forget();
    //     })
    // };

    let reset_light = {
        // let current_theme = current_theme.clone();
        // let theme_storage = theme_storage.clone();
        // let proc_refresh = proc_refresh.clone();
        let theme = theme.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            // if let Some(ref current_theme) = current_theme {
            //     current_theme.set(Some(ThemeData::default_light_theme()));
            // }
            // theme_storage.set(ThemeData::default_light_theme());
            // proc_refresh.emit(());
            if let Some(theme) = &theme {
                theme.reset_light();
            }
        })
    };

    let reset_dark = {
        // let current_theme = current_theme;
        // let theme_storage = theme_storage;
        // let proc_refresh = proc_refresh;
        let theme = theme.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            // if let Some(ref current_theme) = current_theme {
            //     current_theme.set(Some(ThemeData::default_dark_theme()));
            // }
            // theme_storage.set(ThemeData::default_dark_theme());
            // proc_refresh.emit(());
            if let Some(theme) = &theme {
                theme.reset_dark();
            }
        })
    };

    let cycle_emoji = {
        let emojis = emojis.clone();
        let emoji_cycle = emoji_cycle.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let next = emoji_cycle.next();
            emoji_cycle.set(next);
            if next == EmojiState::Enabled {
                emojis.set(true);
            } else if next == EmojiState::Disabled {
                emojis.set(false);
            }
        })
    };

    html! {
        <div class="settings-theme-editor">
            <div class="settings-theme-editor-title">
                <h1>{"Theme Editor"}</h1>
            </div>
            <div class="settings-theme-color-editors">
                {
                    // if *refresh_val {
                    if let Some(theme) = theme {
                        html! {
                                <>
                                    <ColorEditor label="Primary Color" field={theme.primary_color.clone()} position={Position::First} />
                                    <ColorEditor label="Secondary Color" field={theme.secondary_color.clone()} position={Position::Middle} />
                                    <ColorEditor label="Border Color" field={theme.border_color.clone()} position={Position::Middle} />
                                    <ColorEditor label="Text Color" field={theme.text_color.clone()} position={Position::Middle} />
                                    <ColorEditor label="Secondary Text Color" field={theme.secondary_text_color.clone()} position={Position::Middle} />
                                    <ColorEditor label="Link Color" field={theme.link_color.clone()} position={Position::Middle} />
                                    <ColorEditor label="Bluetext Color" field={theme.bluetext_color.clone()} position={Position::Middle} />
                                    <ColorEditor label="Peetext Color" field={theme.peetext_color.clone()} position={Position::Middle} />
                                    <ColorEditor label="Topic Color" field={theme.topic_color.clone()} position={Position::Middle} />
                                    <ColorEditor label="Error Color" field={theme.error_color.clone()} position={Position::Last} />
                                    //<SizeEditor label="Border Width" field="border_width" />
                                    //<BorderTypeEditor label="Border Type" field="border_type" />
                                    // <ColorEditor label="Post Link Valid Color" field="post_link_valid_color" />
                                    // <ColorEditor label="Post Link Unloaded Color" field="post_link_unloaded_color" />
                                    // <ColorEditor label="Post Link Invalid Color" field="post_link_invalid_color" position={Position::Last}/>
                                    //<SizeEditor label="Edge Padding" field="edge_padding" />
                                    //<TimeEditor label="Animation Speed" field="animation_speed" />
                                    //<SizeEditor label="Border Radius" field="border_radius" />
                                </>
                            }
                    } else {
                        html! {
                            <div class="settings-theme-color-editors-loading">
                                <h1>{"Loading..."}</h1>
                            </div>
                        }
                    }
                }
            </div>
            <div class="settings-theme-reset">
                <a onclick={reset_light}>{ if emojis.unwrap_or(true) { "" } else { "Reset To Light Theme" } }</a>
                <span>{" | "}</span>
                <a  onclick={reset_dark}>{ if emojis.unwrap_or(true) { "" } else { "Reset To Dark Theme" } }</a>
                <span>{" | "}</span>
                <a onclick={cycle_emoji}>{ emoji_cycle.string() }</a>
            </div>
        </div>
    }
}

#[derive(Clone, Default, PartialEq, Debug)]
pub enum Position {
    First,
    #[default]
    Middle,
    Last,
}

impl Position {
    pub fn get_id(&self) -> AttrValue {
        match self {
            Self::First => "first",
            Self::Middle => "middle",
            Self::Last => "last",
        }
        .into()
    }
}

#[derive(Clone, PartialEq, Debug, Copy)]
pub enum EmojiState {
    Enabled,
    Conf1,
    Conf2,
    Conf3,
    Conf4,
    Conf5,
    Conf6,
    Conf7,
    Conf8,
    Disabled,
}

impl EmojiState {
    pub fn next(&self) -> Self {
        match self {
            Self::Enabled => Self::Conf1,
            Self::Conf1 => Self::Conf2,
            Self::Conf2 => Self::Conf3,
            Self::Conf3 => Self::Conf4,
            Self::Conf4 => Self::Conf5,
            Self::Conf5 => Self::Conf6,
            Self::Conf6 => Self::Conf7,
            Self::Conf7 => Self::Conf8,
            Self::Conf8 => Self::Disabled,
            Self::Disabled => Self::Enabled,
        }
    }

    pub fn string(&self) -> AttrValue {
        match self {
            Self::Enabled => "",
            Self::Conf1 => "",
            Self::Conf2 => "",
            Self::Conf3 => "",
            Self::Conf4 => "",
            Self::Conf5 => "",
            Self::Conf6 => "",
            Self::Conf7 => "󰞧",
            Self::Conf8 => "󰮢󰮢󰮢󰮢󰮢󰮢",
            Self::Disabled => "Emojis Disabled",
        }
        .into()
    }
}
