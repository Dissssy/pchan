use std::sync::Arc;

use common::structs::PushMessage;
use futures::{channel::oneshot, StreamExt};
use yew::prelude::*;

pub struct ServerSentEventHandle {
    inner: UseStateHandle<Vec<PushMessage>>,
}

impl ServerSentEventHandle {
    pub fn get(&mut self) -> Option<PushMessage> {
        let mut cloned = self.inner.to_vec();
        if cloned.is_empty() {
            return None;
        }
        let r = cloned.pop();
        self.inner.set(cloned);
        r
    }
}

#[hook]
pub fn use_server_sent_event(
    nav_info: Option<(String, i64)>,
    event_types: Vec<&'static str>,
) -> ServerSentEventHandle {
    let inner = use_state(Vec::new);
    let passer = use_state(|| None);

    {
        let inner = inner.clone();
        let passer = passer.clone();
        use_effect_with_deps(
            move |(inner, passer)| {
                let passed = (**passer).clone();
                if let Some(passed) = passed {
                    let mut cloned = inner.to_vec();
                    cloned.push(passed);
                    inner.set(cloned);
                }
                passer.set(None);
                || {}
            },
            (inner, passer),
        );
    }

    {
        let passer = passer.setter();
        use_effect_with_deps(
            move |nav_info| {
                let nav_info = nav_info.clone();
                let (canceler, mut cancel) = oneshot::channel::<()>();
                wasm_bindgen_futures::spawn_local(async move {
                    gloo::console::info!("event source spawned");
                    let mut stream =
                        match gloo_net::eventsource::futures::EventSource::new(&match nav_info {
                            Some((discrim, thread)) => {
                                format!("/api/v1/board/{}/thread/{}/notifications", discrim, thread)
                            }
                            None => "/api/v1/notifications".to_owned(),
                        }) {
                            Ok(s) => s,
                            Err(e) => {
                                log::error!("failed to create event source: {:?}", e);
                                return;
                            }
                        };

                    let subs = event_types
                        .iter()
                        .cloned()
                        .flat_map(|e| stream.subscribe(e))
                        .collect::<Vec<_>>();

                    let mut all = futures::stream::select_all(subs);

                    loop {
                        futures::select! {
                            v = all.next() => {
                                if let Some(Ok((event_type, msg))) = v {
                                    use gloo::utils::format::JsValueSerdeExt;
                                    if let Some(event) = match event_type.as_str() {
                                        "open" => Some(PushMessage::Open),
                                        "new_post" => serde_json::from_str(
                                            msg.data()
                                                .into_serde::<String>()
                                                .unwrap_or_default()
                                                .as_str(),
                                        )
                                        .map(|p: common::structs::SafePost| PushMessage::NewPost(Arc::new(p)))
                                        .map_err(|e| {
                                            gloo::console::error!(format!("failed to parse new_post: {:?}", e))
                                        })
                                        .ok(),
                                        "close" => Some(PushMessage::Close),
                                        miss => {
                                            gloo::console::warn!(format!("unknown event type: `{}`", miss));
                                            None
                                        }
                                    } {
                                        gloo::console::info!(format!("got event: {:?}", event));
                                        passer.set(Some(event));
                                    }
                                }
                            }
                            _ = cancel => {
                                break;
                            }
                        }
                    }
                    gloo::console::info!("event source canceled");
                });

                || {
                    canceler.send(()).ok();
                }
            },
            nav_info,
        );
    }
    ServerSentEventHandle { inner }
}
