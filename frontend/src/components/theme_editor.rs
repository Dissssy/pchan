use gloo_timers::callback;
use yew::prelude::*;
use yew_hooks::prelude::*;

use crate::theme_data::ThemeData;

use super::editors::color_editor::ColorEditor;

#[function_component]
pub fn ThemeEditor() -> Html {
    let emojis = use_local_storage::<bool>("emojis".to_owned());

    let emoji_cycle = use_state(|| {
        if emojis.unwrap_or(true) {
            EmojiState::Enabled
        } else {
            EmojiState::Disabled
        }
    });

    let current_theme = use_context::<UseStateHandle<Option<ThemeData>>>();
    let theme_storage = use_local_storage::<ThemeData>("theme".to_owned());
    let refresh_val = use_state(|| true);

    let proc_refresh = {
        let refresh_val = refresh_val.clone();
        Callback::from(move |()| {
            refresh_val.set(false);
            // wait for 0.1 seconds
            let refresh_val = refresh_val.clone();
            callback::Timeout::new(10, move || {
                refresh_val.set(true);
            })
            .forget();
        })
    };

    let reset_light = {
        let current_theme = current_theme.clone();
        let theme_storage = theme_storage.clone();
        let proc_refresh = proc_refresh.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            if let Some(ref current_theme) = current_theme {
                current_theme.set(Some(ThemeData::default_light_theme()));
            }
            theme_storage.set(ThemeData::default_light_theme());
            proc_refresh.emit(());
        })
    };

    let reset_dark = {
        let current_theme = current_theme;
        let theme_storage = theme_storage;
        let proc_refresh = proc_refresh;
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            if let Some(ref current_theme) = current_theme {
                current_theme.set(Some(ThemeData::default_dark_theme()));
            }
            theme_storage.set(ThemeData::default_dark_theme());
            proc_refresh.emit(());
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
                    if *refresh_val {
                        html! {
                                <>
                                    <ColorEditor label="Primary Color" field="primary_color" position={Position::First}/>
                                    <ColorEditor label="Secondary Color" field="secondary_color" />
                                    <ColorEditor label="Topic Color" field="topic_color" />
                                    <ColorEditor label="Bluetext Color" field="bluetext_color" />
                                    <ColorEditor label="Peetext Color" field="peetext_color" />
                                    <ColorEditor label="Border Color" field="border_color" />
                                    <ColorEditor label="Error Color" field="error_color" />
                                    <ColorEditor label="Text Color" field="text_color" />
                                    <ColorEditor label="Secondary Text Color" field="secondary_text_color" />
                                    //<SizeEditor label="Border Width" field="border_width" />
                                    //<BorderTypeEditor label="Border Type" field="border_type" />
                                    <ColorEditor label="Link Color" field="link_color" />
                                    <ColorEditor label="Post Link Valid Color" field="post_link_valid_color" />
                                    <ColorEditor label="Post Link Unloaded Color" field="post_link_unloaded_color" />
                                    <ColorEditor label="Post Link Invalid Color" field="post_link_invalid_color" position={Position::Last}/>
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
                <a onclick={reset_light}>{ if emojis.unwrap_or(true) { "☀️" } else { "Reset To Light Theme" } }</a>
                <span>{" | "}</span>
                <a  onclick={reset_dark}>{ if emojis.unwrap_or(true) { "🌑" } else { "Reset To Dark Theme" } }</a>
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
    pub fn get_id(&self) -> &str {
        match self {
            Position::First => "first",
            Position::Middle => "middle",
            Position::Last => "last",
        }
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
            EmojiState::Enabled => EmojiState::Conf1,
            EmojiState::Conf1 => EmojiState::Conf2,
            EmojiState::Conf2 => EmojiState::Conf3,
            EmojiState::Conf3 => EmojiState::Conf4,
            EmojiState::Conf4 => EmojiState::Conf5,
            EmojiState::Conf5 => EmojiState::Conf6,
            EmojiState::Conf6 => EmojiState::Conf7,
            EmojiState::Conf7 => EmojiState::Conf8,
            EmojiState::Conf8 => EmojiState::Disabled,
            EmojiState::Disabled => EmojiState::Enabled,
        }
    }

    pub fn string(&self) -> &'static str {
        match self {
            EmojiState::Enabled => "👍",
            EmojiState::Conf1 => "🤔",
            EmojiState::Conf2 => "😬",
            EmojiState::Conf3 => "😳",
            EmojiState::Conf4 => "😨",
            EmojiState::Conf5 => "😱",
            EmojiState::Conf6 => "🤢",
            EmojiState::Conf7 => "🤮",
            EmojiState::Conf8 => "😵",
            EmojiState::Disabled => "Emojis Disabled",
        }
    }
}
