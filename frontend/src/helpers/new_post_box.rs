use common::structs::{CreatePost, CreateThread};

use yew::prelude::*;

use crate::{
    api::{ReplyContext, ThreadContext},
    on_change_to_string,
};

#[function_component]
pub fn PostBox(props: &Props) -> Html {
    let post_text = props.post_text.clone();
    let post_error = use_state(|| None);

    let mvpost_text = post_text.clone();
    let oninput_post = Callback::from(move |e: InputEvent| {
        if let Some(input) = on_change_to_string(e) {
            mvpost_text.set(input);
        } else {
            gloo::console::log!("no input");
        }
    });

    let post_topic = use_state(String::new);

    let mvpost_topic = post_topic.clone();
    let oninput_topic = Callback::from(move |e: InputEvent| {
        if let Some(input) = on_change_to_string(e) {
            mvpost_topic.set(input);
        } else {
            gloo::console::log!("no input");
        }
    });

    let expanded = use_state(|| false);

    let mvexpanded = expanded.clone();
    let onclick_expand = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        mvexpanded.set(!*mvexpanded);
    });

    let spoilered = use_state(|| false);

    let mvspoilered = spoilered.clone();
    let onclick_spoiler = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        mvspoilered.set(!*mvspoilered);
    });

    let name = use_state(|| match crate::get_name() {
        Ok(v) => v,
        Err(e) => {
            gloo::console::log!(format!("error getting name: {:?}", e));
            None
        }
    });

    let mvname = name.clone();
    let oninput_name = Callback::from(move |e: InputEvent| {
        if let Some(input) = on_change_to_string(e) {
            if let Err(e) = crate::set_name(input.clone()) {
                gloo::console::log!(format!("error setting name: {:?}", e));
            }
            mvname.set(if input.is_empty() { None } else { Some(input) });
        } else {
            gloo::console::log!("no input");
        }
    });

    let file = use_state(|| None);

    let mvfile = file.clone();
    let onchange_file = Callback::from(move |e: Event| {
        use wasm_bindgen::JsCast;
        if let Some(input) = e
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
            .and_then(|v| v.files())
        {
            if input.length() > 0 {
                mvfile.set(input.get(0));
            }
        }
    });

    let pending = use_state(|| false);

    let mvpost_text = post_text.clone();
    let mvname = name.clone();
    let mvexpanded = expanded.clone();
    let mvprops = props.clone();
    let mvpost_error = post_error.clone();
    let mvtopic = post_topic.clone();
    let mvspoilered = spoilered.clone();
    let submit_post = Callback::from(move |_| {
        if !*pending {
            pending.set(true);
        } else {
            gloo::console::log!("already pending");
            return;
        }
        let post_text = mvpost_text.clone();
        let name = mvname.clone();
        let file = file.clone();
        let expanded = mvexpanded.clone();
        let pending = pending.clone();
        let post_error = mvpost_error.clone();
        let spoilered = mvspoilered.clone();
        let props = mvprops.clone();
        let post_topic = mvtopic.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let file_to_post = match crate::API
                .lock()
                .await
                .upload_file(file.clone(), *spoilered)
                .await
            {
                Ok(file) => file,
                Err(e) => {
                    gloo::console::log!(format!("error uploading file: {:?}", e));
                    pending.set(false);
                    return;
                }
            };
            let post = CreatePost {
                file: file_to_post,
                content: (*post_text).clone(),
                author: (*name).clone(),
            };
            let p = match props.thread_id {
                Some((ref t, _)) => {
                    let context = ReplyContext {
                        board_discriminator: props.board_discriminator.clone(),
                        thread_id: t.clone(),
                    };

                    crate::API.lock().await.post_reply(post, context).await
                }
                None => {
                    let context = ThreadContext {
                        board_discriminator: props.board_discriminator.clone(),
                    };
                    let thred = CreateThread {
                        post,
                        topic: (*post_topic).clone(),
                    };
                    crate::API.lock().await.post_thread(thred, context).await
                }
            };
            match p {
                Ok(p) => {
                    match props.thread_id {
                        Some((_, callback)) => {
                            callback.emit(());
                            post_text.set(String::new());
                            file.set(None);
                            expanded.set(false);
                        }
                        None => {
                            let url =
                                format!("/{}/thread/{}", props.board_discriminator, p.post_number);
                            match web_sys::window() {
                                Some(w) => match w.location().set_href(&url) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        let err = format!("{e:?}");
                                        gloo::console::log!(err.clone());
                                        post_error.set(Some(err));
                                    }
                                },
                                None => {
                                    gloo::console::log!("no window");
                                }
                            }
                        }
                    };
                }
                Err(e) => {
                    let err = format!("{e:?}");
                    gloo::console::log!(err.clone());
                    post_error.set(Some(err));
                }
            }
            pending.set(false);
        });
    });

    html! {
        <div class="submission-box">
            <div class="submission-box-header">
                <a href="#" onclick={onclick_expand}>
                    <p>{ match props.thread_id {
                        Some(_) => "Reply".to_owned(),
                        None => "New Thread".to_owned(),
                    }}{" "}{
                        if *expanded {
                            "[-]"
                        } else {
                            "[+]"
                        }
                    }</p>
                </a>
            </div>
            {
                if *expanded {
                    html!{
                        <div class="submission-box-inputs">
                            <div class="submission-box-text-inputs">
                                <div class="name-and-topic">
                                    <div class="submission-box-name-input">
                                        <label for="name-input">{"Name"}</label>
                                        <input type="text" id="name-input" class="custom-select" name="name-input" oninput={oninput_name} value={
                                            match &*name {
                                                Some(n) => n.clone(),
                                                None => "".to_owned(),
                                            }
                                        }/>
                                    </div>
                                    {
                                        if props.thread_id.is_none() {
                                            html! {
                                                <div class="submission-box-topic-input">
                                                    <label for="topic-input">{"Topic"}</label>
                                                    <input type="text" id="topic-input" class="custom-select" name="topic-input" oninput={oninput_topic} value={
                                                        (*post_topic).clone()
                                                    }/>
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                                <div class="submission-box-post-input">
                                    <textarea id="post-input" class="custom-select" name="post-input" oninput={oninput_post} value={
                                        (*post_text).clone()
                                    }>
                                    </textarea>
                                </div>
                            </div>
                            <div class="submission-box-file-input">
                                <input type="button" id="spoiler-button" class="custom-select" name="spoiler-button" onclick={onclick_spoiler} value={
                                    if *spoilered {
                                        "Spoiler"
                                    } else {
                                        "-"
                                    }
                                }/>
                                <label for="file-input">{"File"}</label>
                                <input type="file" id="file-input" name="file-input" onchange={onchange_file}/>
                            </div>
                            <div class="submission-box-submit">
                                <button class="custom-select" onclick={submit_post}>{ match props.thread_id {
                                    Some(_) => "Reply",
                                    None => "New Thread",
                                }}</button>
                                {
                                    match *post_error {
                                        Some(ref e) => html! {
                                            <span class="submission-box-post-error">{e}</span>
                                        },
                                        None => html! {}
                                    }
                                }
                            </div>
                        </div>
                    }
                } else {
                    html!{}
                }
            }

        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub board_discriminator: String,
    pub thread_id: Option<(String, Callback<()>)>,
    pub post_text: UseStateHandle<String>,
}
