use common::structs::CreatePost;

use yew::prelude::*;

use crate::{
    api::{ReplyContext, ThreadContext},
    on_change_to_string,
};

#[function_component]
pub fn PostBox(props: &Props) -> Html {
    // a post box component that will be used to create new posts and reply to existing posts.
    // there will be a text area for your name, a text area for the post, a file upload button, and a submit button.
    // the post box will be used on the board page, and the thread page.

    let post_text = use_state(String::new);
    let post_error = use_state(|| None);

    let mvpost_text = post_text.clone();
    let onchange_post = Callback::from(move |e: Event| {
        if let Some(input) = on_change_to_string(e) {
            mvpost_text.set(input);
        } else {
            gloo::console::log!("no input");
        }
    });
    let starter_name = match crate::get_name() {
        Ok(v) => v,
        Err(e) => {
            gloo::console::log!(format!("error getting name: {:?}", e));
            None
        }
    };
    let name = use_state(|| starter_name.clone());

    let mvname = name.clone();
    let onchange_name = Callback::from(move |e: Event| {
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

    // let mvpost_text = post_text;
    // let mvname = name;
    // let mvfile = file;
    let mvprops = props.clone();
    let mvpost_error = post_error.clone();
    let submit_post = Callback::from(move |_| {
        if !*pending {
            pending.set(true);
        } else {
            gloo::console::log!("already pending");
            return;
        }
        let post_text = post_text.clone();
        let name = name.clone();
        let file = file.clone();
        let pending = pending.clone();
        let post_error = mvpost_error.clone();
        let props = mvprops.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let file_to_post = match crate::API.lock().await.upload_file(file.clone()).await {
                Ok(file) => file,
                Err(e) => {
                    gloo::console::log!(format!("error uploading file: {:?}", e));
                    pending.set(false);
                    return;
                }
            };
            // if we are replying to a thread, we need to send the thread id to the server. otherwise we need to use a different data structure.
            let post = CreatePost {
                file: file_to_post,
                content: (*post_text).clone(),
                author: (*name).clone(),
            };
            let p = match props.thread_id {
                Some((ref t, _)) => {
                    // copmment
                    let context = ReplyContext {
                        board_discriminator: props.board_discriminator.clone(),
                        thread_id: t.clone(),
                    };

                    crate::API.lock().await.post_reply(post, context).await
                }
                None => {
                    // commbent
                    let context = ThreadContext {
                        board_discriminator: props.board_discriminator.clone(),
                    };
                    crate::API.lock().await.post_thread(post, context).await
                }
            };
            match p {
                Ok(p) => {
                    // combendnt
                    match props.thread_id {
                        Some((_, _callback)) => {
                            // initiate a manual post reload using the post thingy!!!
                            // _callback.emit(());
                            // nevermind i cant figure out how to clear the text inputs, reload the page :(

                            match web_sys::window() {
                                Some(w) => match w.location().reload() {
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
                        None => {
                            let url = format!("/{}/{}", props.board_discriminator, p.post_number);
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
                <p>{ match props.thread_id {
                    // Some(ref t) => format!("Reply >>{t}"),
                    Some(_) => "Reply".to_owned(),
                    None => "New Thread".to_owned(),
                }}</p>
            </div>
            <div class="submission-box-text-inputs">
                <div class="submission-box-name-input">
                    <label for="name-input">{"Name"}</label>
                    <input type="text" id="name-input" name="name-input" onchange={onchange_name} value={
                        match starter_name {
                            Some(ref s) => s.clone(),
                            None => "".to_owned(),
                        }
                    }/>
                </div>
                <div class="submission-box-post-input">
                    <label for="post-input">{"Post"}</label>
                    <textarea id="post-input" name="post-input" onchange={onchange_post}>
                    </textarea>
                </div>
            </div>
            <div class="submission-box-file-input">
                <label for="file-input">{"File"}</label>
                <input type="file" id="file-input" name="file-input" onchange={onchange_file}/>
            </div>
            <div class="submission-box-submit">
                <button onclick={submit_post}>{ match props.thread_id {
                    Some(_) => "Reply",
                    None => "New Thread",
                }}</button>
                {
                    match *post_error {
                        Some(ref e) => html! {
                            <p class="submission-box-post-error">{e}</p>
                        },
                        None => html! {}
                    }
                }
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub board_discriminator: String,
    pub thread_id: Option<(String, Callback<()>)>,
    // pub starter_text: Option<String>,
}
