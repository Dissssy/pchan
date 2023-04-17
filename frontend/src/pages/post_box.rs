use gloo_storage::Storage;
use serde::{Deserialize, Serialize};

use yew::prelude::*;

use crate::on_change_to_string;

#[function_component]
pub fn PostBox(props: &Props) -> Html {
    // a post box component that will be used to create new posts and reply to existing posts.
    // there will be a text area for your name, a text area for the post, a file upload button, and a submit button.
    // the post box will be used on the board page, and the thread page.
    let token = use_state(|| None);

    let mvtoken = token.clone();
    if token.is_none() {
        wasm_bindgen_futures::spawn_local(async move {
            let token = gloo_net::http::Request::get("/api/v1/")
                .send()
                .await
                .unwrap()
                .json::<String>()
                .await
                .unwrap();
            mvtoken.set(Some(token));
        });
    }

    let post_text = use_state(|| "".to_string());

    let mvpost_text = post_text.clone();
    let onchange_post = Callback::from(move |e: Event| {
        if let Some(input) = on_change_to_string(e) {
            mvpost_text.set(input);
        } else {
            gloo::console::log!("no input");
        }
    });

    let name = use_state(|| None);

    let mvname = name.clone();
    let onchange_name = Callback::from(move |e: Event| {
        if let Some(input) = on_change_to_string(e) {
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
    let mvfile = file.clone();
    let mvtoken = token.clone();
    let mvprops = props.clone();
    let submit_post = Callback::from(move |_| {
        let mvtoken = mvtoken.clone();
        if !*pending {
            pending.set(true);
        } else {
            gloo::console::log!("already pending");
            return;
        }
        if let Some(ref token) = *mvtoken {
            let token = token.clone();
            let mvvpost_text = mvpost_text.clone();
            let mvvname = mvname.clone();
            let mvvfile = mvfile.clone();
            let pclone = pending.clone();
            let mvprops = mvprops.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let file_to_post = if let Some(f) = &*mvvfile {
                    gloo::console::log!("uploading {:?}", f);
                    let form_data = web_sys::FormData::new().unwrap();
                    form_data.append_with_blob("file", f).unwrap();

                    let res = gloo_net::http::Request::post("/api/v1/file")
                        .header("authorization", &format!("Bearer {token}"))
                        .body(&form_data)
                        .send()
                        .await
                        .unwrap();
                    let file_id = res.json::<String>().await.unwrap();
                    if file_id.contains(' ') {
                        gloo::console::log!("file upload failed");
                        pclone.set(false);
                        return;
                    }
                    Some(file_id)
                } else {
                    None
                };
                let name_to_post = &*mvvname;
                let content_to_post = &*mvvpost_text;

                // if we are replying to a thread, we need to send the thread id to the server. otherwise we need to use a different data structure.
                let data_to_send = PostReply {
                    image: file_to_post,
                    content: content_to_post.clone(),
                    author: name_to_post.clone(),
                };
                let board_discriminator = mvprops.board_discriminator.clone();
                if let Some(thread_id) = mvprops.thread_id {
                    // we are replying to a thread, post to /api/v1/board/{board_discriminator}/{thread_id}
                    let url = format!("/api/v1/board/{board_discriminator}/{thread_id}");

                    let res = gloo_net::http::Request::post(&url)
                        .header("authorization", &format!("Bearer {token}"))
                        .json(&data_to_send)
                        .unwrap();
                    gloo::console::log!(format!("posting to {:?}", res));
                    let res = res.send().await.unwrap();

                    let respons = res.text().await.unwrap();
                    gloo::console::log!("response: {:?}", respons);
                } else {
                    // we are creating a new thread, post to /api/v1/board/{board_discriminator}
                    let data_to_send = PostThread { post: data_to_send };
                    let url = format!("/api/v1/board/{board_discriminator}");

                    let res = gloo::net::http::Request::post(&url)
                        .header("authorization", &format!("Bearer {token}"))
                        .json(&data_to_send)
                        .unwrap()
                        .send()
                        .await
                        .unwrap();

                    let respons = res.text().await.unwrap();
                    gloo::console::log!("response: {:?}", respons);
                }
                pclone.set(false);
            });
        }
    });

    html! {
        <div class="submission-box">
            <div class="submission-box-header">
                <h1>{ match props.thread_id {
                    Some(_) => "Reply",
                    None => "New Thread",
                }}</h1>
            </div>
            <div class="submission-box-text-inputs">
                <div class="submission-box-name-input">
                    <label for="name-input">{"Name"}</label>
                    <input type="text" id="name-input" name="name-input" onchange={onchange_name}/>
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
            </div>
        </div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub board_discriminator: String,
    pub thread_id: Option<String>,
    pub starter_text: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Value {
    #[serde(rename = "textContent")]
    pub value: String,
}

#[derive(Serialize, Debug)]
pub struct PostReply {
    pub image: Option<String>,
    pub content: String,
    pub author: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct PostThread {
    pub post: PostReply,
}
