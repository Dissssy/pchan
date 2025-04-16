use crate::{
    api::ApiError,
    components::{OffsetType, ParentOffset},
    ApiContext,
};

use super::HoveredOrExpandedState;
use yew::prelude::*;
use yew_hooks::use_local_storage;

#[function_component]
pub fn File(props: &Props) -> Html {
    let share_state = use_state(|| ShareState::None);
    let file_state = use_state(|| HoveredOrExpandedState::None);
    let spoiler = props.file.claimed.spoiler;

    let api_ctx = use_context::<Option<ApiContext>>().flatten();

    let emojis = use_local_storage::<bool>("emojis".to_owned()).unwrap_or(true);

    let parent_offset = use_context::<Option<ParentOffset>>()
        .flatten()
        .unwrap_or_default();

    let screen_height = web_sys::window()
        .and_then(|w| w.inner_height().ok())
        .and_then(|h| h.as_f64())
        .unwrap_or(0.0);

    // let tfile_state = file_state.clone();
    // let on_click_with_expanded = Callback::from(move |e: MouseEvent| {
    //     e.prevent_default();
    //     tfile_state.set(match *tfile_state {
    //         HoveredOrExpandedState::Expanded => {
    //             if spoiler {
    //                 HoveredOrExpandedState::None
    //             } else {
    //                 HoveredOrExpandedState::Hovered
    //             }
    //         }
    //         _ => HoveredOrExpandedState::Expanded,
    //     });
    // });

    let tfile_state = file_state.clone();
    let on_click_without_expanded = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        tfile_state.set(match *tfile_state {
            HoveredOrExpandedState::Expanded {
                x: _,
                y: _,
                offset: _,
            } => HoveredOrExpandedState::None,
            _ => {
                let base_pos = e.client_y() as f64;

                let normalized_pos = base_pos / screen_height;

                let offset = if normalized_pos < 0.33 {
                    OffsetType::Bottom
                } else if normalized_pos > 0.66 {
                    OffsetType::Top
                } else {
                    OffsetType::Center
                };

                HoveredOrExpandedState::Expanded {
                    x: e.page_x() - parent_offset.x,
                    y: e.page_y() - parent_offset.y,
                    offset,
                }
            }
        });
    });

    let tfile_state = file_state.clone();
    let on_hover = Callback::from(move |e: MouseEvent| {
        if !spoiler {
            tfile_state.set(match *tfile_state {
                HoveredOrExpandedState::Expanded { x, y, offset } => {
                    HoveredOrExpandedState::Expanded { x, y, offset }
                }
                _ => {
                    let base_pos = e.client_y() as f64;

                    let normalized_pos = base_pos / screen_height;

                    let offset = if normalized_pos < 0.33 {
                        OffsetType::Bottom
                    } else if normalized_pos > 0.66 {
                        OffsetType::Top
                    } else {
                        OffsetType::Center
                    };

                    HoveredOrExpandedState::Hovered {
                        x: e.page_x() - parent_offset.x,
                        y: e.page_y() - parent_offset.y,
                        offset,
                    }
                }
            });
        }
    });

    let tfile_state = file_state.clone();
    let on_mouseoff = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        tfile_state.set(match *tfile_state {
            HoveredOrExpandedState::Expanded { x, y, offset } => {
                HoveredOrExpandedState::Expanded { x, y, offset }
            }
            _ => HoveredOrExpandedState::None,
        });
    });

    let on_click_share = {
        let path = props.file.claimed.path.clone();
        let share_state = share_state.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let api_ctx = api_ctx.clone();

            if *share_state != ShareState::Pending {
                share_state.set(ShareState::Pending);
                let path = path.clone();
                let share_state = share_state.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    let path = match &api_ctx {
                        Some(api_ctx) => match &api_ctx.api {
                            Ok(api) => api.share_file(&path).await,
                            Err(e) => Err(ApiError::Other(AttrValue::from(format!("{}", **e)))),
                        },
                        _ => Err(ApiError::Other(AttrValue::from("No API context"))),
                    };

                    let path = match path {
                        Ok(path) => path,
                        Err(e) => {
                            share_state.set(ShareState::Error(format!("{}", *e)));
                            return;
                        }
                    };

                    if path.contains(' ') {
                        share_state.set(ShareState::Error(path));
                        return;
                    }

                    if let Some(window) = web_sys::window() {
                        if let Some(clip) = window.navigator().clipboard() {
                            let _ = clip.write_text(&format!("{}{}", env!("URL"), path));
                            share_state.set(ShareState::Copied);
                        } else {
                            share_state
                                .set(ShareState::Error("Clipboard not available".to_string()));
                        }
                    } else {
                        share_state.set(ShareState::Error("Window not available".to_string()));
                    }
                });
            }
        })
    };

    html! {
        <div class="post-file-container" draggable="false">
            // <div class="post-file-header">
                <span class="post-expand">
                    <a onclick={on_click_without_expanded.clone()} onmousemove={on_hover} onmouseleave={on_mouseoff} draggable="false">
                        {
                            match *file_state {
                                HoveredOrExpandedState::None => {
                                    (if emojis { "" } else { "Expand" }).to_owned()
                                }
                                HoveredOrExpandedState::Hovered {
                                    x: _,
                                    y: _,
                                    offset: _,
                                } => {
                                    (if emojis { "" } else { "Hovered" }).to_owned()
                                }
                                HoveredOrExpandedState::Expanded { x: _, y: _, offset: _ } => {
                                    (if emojis { "" } else { "Expanded" }).to_owned()
                                }
                            }
                            // if !(*file_state == HoveredOrExpandedState::None)  {
                            //     format!("[-]{}", if *file_state == HoveredOrExpandedState::Expanded { " (held)" } else { "" })
                            // } else {
                            //     "[+]".to_owned()
                            // }
                        }
                    </a>
                </span>
                <span class="post-hash" title={format!("Hash: {}", props.file.claimed.hash.clone())}>
                    { if emojis { "" } else { "Hash" }}
                </span>
                if props.file.claimed.path.contains("/audio/") || props.file.claimed.path.contains("/video/") {
                    <span class="might-have-sound-indicator" >
                        {
                            match (emojis, props.file.claimed.path.contains("/audio/")) {
                                (true, true) => "",
                                (true, false) => "󰸬",
                                (false, true) => "Audio",
                                (false, false) => "Video",
                            }
                        }
                    </span>
                }
                <span class="post-share" title={match *share_state {
                    ShareState::Error(ref e) => format!("Error: {}", e),
                    _ => "Share this file (link will be copied to clipboard)".to_string()
                }}>
                    <a onclick={on_click_share} draggable="false">
                        { match (&*share_state, emojis) {
                            (ShareState::None, true) => "󰤲",
                            (ShareState::None, false) => "Share",
                            (ShareState::Pending, true) => "",
                            (ShareState::Pending, false) => "Creating...",
                            (ShareState::Copied, true) => "",
                            (ShareState::Copied, false) => "Copied",
                            (ShareState::Error(_), true) => "",
                            (ShareState::Error(_), false) => "Error",
                        } }
                    </a>
                </span>
            // </div>
            <div class="post-file-contents">
                <a href={props.file.claimed.path.clone()} onclick={on_click_without_expanded} draggable="false">
                {
                    match *file_state {
                        HoveredOrExpandedState::None => {
                            html! {
                                <div class="post-file-thumbnail">
                                    <img src={props.file.claimed.thumbnail.clone()} />
                                </div>
                            }
                        }
                        HoveredOrExpandedState::Hovered {
                            x,
                            y,
                            offset,
                        } => {
                            html! {
                                <>
                                    <img src={props.file.claimed.thumbnail.clone()} />
                                    <div class="floating-image" style={format!("left: calc({}px + 1em) !important; top: calc({}px) !important; position: absolute !important; transform: translateY({}) !important;", x, y, offset.percent())}>
                                        {
                                            file_html(&props.file)
                                        }
                                    </div>
                                </>
                            }
                        }
                        HoveredOrExpandedState::Expanded { x: _, y: _, offset: _ } => {
                            file_html(&props.file)
                        }
                    }
                }
                // <style>
                //     {
                //         file_state.get_style()
                //     }
                // </style>
                </a>
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub file: common::structs::FileInfo,
}

fn file_html(file: &common::structs::FileInfo) -> Html {
    let mimetype = file.claimed.path.replace("/files/", "");
    let mut th = mimetype.split('/');
    let mut mime = th.next();
    let after = th.next();

    if let Some(after) = after {
        if after.contains("svg") {
            mime = Some("svg")
        }
    }

    match mime {
        None => {
            html! {
                <div class="post-media-error">
                    <img src="/res/404.png"/>
                    <a href={file.claimed.path.clone()}>{"Unsupported embed type: None"}</a>
                </div>
            }
        }
        Some(m) => match m {
            "image" => {
                html! {
                    <img src={file.claimed.path.clone()} draggable="false"/>
                }
            }
            "video" => {
                html! {
                    <video autoplay=true loop=true controls=true class="post-media-video" muted=true draggable="false">
                        <source src={file.claimed.path.clone()} />
                    </video>
                }
            }
            "audio" => {
                html! {
                    <audio autoplay=true loop=true controls=true class="post-media-audio" muted=true draggable="false">
                        <source src={file.claimed.path.clone()} />
                    </audio>
                }
            }
            _ => {
                html! {
                    <div class="post-media-error">
                        <a href={file.claimed.path.clone()}>{"---Unsupported embed type, middle click to download---"}</a>
                    </div>
                }
            }
        },
    }
}

#[derive(Clone, PartialEq)]
pub enum ShareState {
    None,
    Pending,
    Copied,
    Error(String),
}
