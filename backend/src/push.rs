use std::{collections::HashMap, convert::Infallible};

use async_stream::stream;
use common::structs::PushMessage;
use futures::Stream;
use warp::sse::Event;

pub struct PushHolder {
    pushes: HashMap<String, Vec<Push>>,
}

pub type Push = tokio::sync::mpsc::UnboundedSender<PushMessage>;
pub type Recv = tokio::sync::mpsc::UnboundedReceiver<PushMessage>;

fn into_event(msg: PushMessage) -> anyhow::Result<Event> {
    Ok(match msg {
        PushMessage::Open => Event::default().event("open"),
        PushMessage::NewPost(post) => Event::default()
            .event("new_post")
            .json_data(post.as_ref())?,
        PushMessage::Close => Event::default().event("close"),
    })
}

impl PushHolder {
    pub fn new() -> Self {
        Self {
            pushes: HashMap::new(),
        }
    }

    pub fn add_push(&mut self, ident: String) -> Recv {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut entry = self.pushes.entry(ident);
        let e = match entry {
            std::collections::hash_map::Entry::Occupied(ref mut e) => e.get_mut(),
            std::collections::hash_map::Entry::Vacant(v) => v.insert(vec![]),
        };
        e.push(tx);
        rx
    }

    pub async fn subscribe(
        &mut self,
        ident: String,
    ) -> impl Stream<Item = Result<Event, Infallible>> {
        let mut rx = self.add_push(ident.clone());
        self.send_to(&[ident], PushMessage::Open);
        stream! {
            while let Some(message) = rx.recv().await {
                yield Ok(match into_event(message) {
                    Ok(event) => event,
                    Err(e) => {
                        Event::default().event("error").data(format!("{:?}", e))
                    }
                })
            }
        }
    }

    pub fn remove_push(&mut self, ident: &String) {
        self.pushes.remove(ident);
    }

    pub fn send_to(&mut self, idents: &[String], message: PushMessage) {
        // attempt to get the list of pushes for each ident (mutable)
        idents.iter().for_each(|ident| {
            if let Some(push) = self.pushes.get_mut(ident) {
                // attempt to send the message to each push, if sending fails, remove the push from the list
                push.retain(|p| p.send(message.clone()).is_ok());
            }
        })
    }
}

impl Default for PushHolder {
    fn default() -> Self {
        Self::new()
    }
}
