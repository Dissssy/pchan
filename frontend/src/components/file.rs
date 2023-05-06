use super::HoveredOrExpandedState;
use yew::prelude::*;
use yew_hooks::use_local_storage;

#[function_component]
pub fn File(props: &Props) -> Html {
    let file_state = use_state(|| HoveredOrExpandedState::None);
    let spoiler = props.file.spoiler;

    let ignore_files = vec!["audio"];

    let emojis = use_local_storage::<bool>("emojis".to_owned()).unwrap_or(true);

    let ignore_mouse = props
        .file
        .path
        .replace("/files/", "")
        .split('/')
        .next()
        .map(|s| ignore_files.contains(&s))
        .unwrap_or(false);

    let tfile_state = file_state.clone();
    let on_click_with_expanded = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        tfile_state.set(match *tfile_state {
            HoveredOrExpandedState::Expanded => HoveredOrExpandedState::Hovered,
            _ => HoveredOrExpandedState::Expanded,
        });
    });

    let tfile_state = file_state.clone();
    let on_click_without_expanded = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        tfile_state.set(match *tfile_state {
            HoveredOrExpandedState::Expanded => HoveredOrExpandedState::None,
            _ => HoveredOrExpandedState::Expanded,
        });
    });

    let tfile_state = file_state.clone();
    let on_hover = Callback::from(move |_e: MouseEvent| {
        if !(spoiler || ignore_mouse) {
            tfile_state.set(match *tfile_state {
                HoveredOrExpandedState::Expanded => HoveredOrExpandedState::Expanded,
                _ => HoveredOrExpandedState::Hovered,
            });
        }
    });

    let tfile_state = file_state.clone();
    let on_mouseoff = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        if !ignore_mouse {
            tfile_state.set(match *tfile_state {
                HoveredOrExpandedState::Expanded => HoveredOrExpandedState::Expanded,
                _ => HoveredOrExpandedState::None,
            });
        }
    });

    html! {
        <div class="post-file-container">
            // <div class="post-file-header">
                <span class="post-expand">
                    <a href="#" onclick={on_click_with_expanded} onmousemove={on_hover} onmouseleave={on_mouseoff} >
                        {
                            match *file_state {
                                HoveredOrExpandedState::None => {
                                    (if emojis { "📁" } else { "Expand" }).to_owned()
                                }
                                HoveredOrExpandedState::Hovered => {
                                    (if emojis { "📂" } else { "Hovered" }).to_owned()
                                }
                                HoveredOrExpandedState::Expanded => {
                                    (if emojis { "📄" } else { "Expanded" }).to_owned()
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
                <span class="post-hash" title={format!("Hash: {}", props.file.hash.clone())}>
                    { if emojis { "ℹ️" } else { "Hash" }}
                </span>
            // </div>
            <div class="post-file">
                <a href={props.file.path.clone()} onclick={on_click_without_expanded} >
                {
                    match *file_state {
                        HoveredOrExpandedState::None => {
                            html! {
                                <img src={props.file.thumbnail.clone()} />
                            }
                        }
                        HoveredOrExpandedState::Hovered => {
                            html! {
                                <>
                                    <img src={props.file.thumbnail.clone()} />
                                    <div class="floating-image">
                                        {
                                            file_html(&props.file)
                                        }
                                    </div>
                                </>
                            }
                        }
                        HoveredOrExpandedState::Expanded => {
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
    let mimetype = file.path.replace("/files/", "");
    let mime = mimetype.split('/').next();

    match mime {
        None => {
            html! {
                <div class="post-media-error">
                    <img src="/res/404.png"/>
                    <a href={file.path.clone()}>{"Unsupported embed type: None"}</a>
                </div>
            }
        }
        Some(m) => match m {
            "image" => {
                html! {
                    <img src={file.path.clone()} />
                }
            }
            "video" => {
                html! {
                    <video autoplay=true loop=true controls=true class="post-media-video">
                        <source src={file.path.clone()} />
                    </video>
                }
            }
            "audio" => {
                html! {
                    <audio autoplay=true loop=true controls=true class="post-media-audio">
                        <source src={file.path.clone()} />
                    </audio>
                }
            }
            _ => {
                html! {
                    <div class="post-media-error">
                        <a href={file.path.clone()}>{"Unsupported embed type: "}{m}</a>
                    </div>
                }
            }
        },
    }
}
