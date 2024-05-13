use common::structs::{PushMessage, SafePost};
use yew::prelude::*;
use yew_router::prelude::{use_navigator, use_route};

use crate::{components::richtext::SpoilableText, hooks::use_server_sent_event, BaseRoute};

#[function_component]
pub fn NotificationBox() -> Html {
    let mut handle = use_server_sent_event(None, vec!["new_post"]);
    let pop = use_state(|| None);
    let open = use_state(|| false);

    let active_notifications = use_state(Vec::<NotificationInfo>::new);

    {
        let pop = pop.clone();
        let active_notifications = active_notifications.clone();
        let open = open.clone();
        use_effect_with(
            (pop, active_notifications, open),
            move |(pop, active_notifications, open)| {
                if let Some(tpop) = **pop {
                    let mut cloned = active_notifications.to_vec();
                    cloned.remove(tpop);
                    if cloned.is_empty() {
                        open.set(false);
                    }
                    active_notifications.set(cloned);
                    pop.set(None);
                }
            },
        );
    }

    let route = use_route::<BaseRoute>();

    if let Some(thing) = handle.get() {
        let mut cloned = active_notifications.to_vec();
        match thing {
            PushMessage::Open => {}
            PushMessage::NewPost(post) => {
                if route
                    != Some(BaseRoute::ThreadPage {
                        board_discriminator: post.board_discriminator.clone(),
                        thread_id: post.thread_post_number.to_string(),
                    })
                {
                    cloned.push(post.as_ref().clone().into());
                }
            }
            PushMessage::Close => {}
        }
        active_notifications.set(cloned);
    }

    let disregard = {
        let pop = pop;
        Callback::from(move |i: usize| {
            pop.set(Some(i));
        })
    };
    let navigator = use_navigator();
    let navigate_to = {
        let navigator = navigator;
        Callback::from(move |path: Path| {
            if let Some(nav) = navigator.as_ref() {
                match path {
                    Path::Board(discrim) => {
                        nav.push(&BaseRoute::BoardPage {
                            board_discriminator: discrim,
                        });
                    }
                    Path::Thread(discrim, thread) => {
                        nav.push(&BaseRoute::ThreadPage {
                            board_discriminator: discrim,
                            thread_id: thread.to_string(),
                        });
                    }
                    Path::Arbitrary(path) => {
                        // set href someday
                        gloo::console::warn!(format!(
                            "Arbitrary paths not implemented yet: `{}`",
                            path
                        ))
                    }
                }
            }
        })
    };

    html! {
        if !active_notifications.is_empty() {
            <div class="notification-popup">
                <span href="#" class="notification-popup-toggle" onclick={ let open = open.clone(); Callback::from(move |_| open.set(!*open)) }>{ if *open { "" } else { "" } }</span>
                if *open {
                    <div class="notification-list">
                        {
                            for active_notifications.iter().enumerate().map(|(i, n)| {

                                html! {
                                    <div class="notification">
                                        <div class="notification-header">
                                            <span onclick={ disregard.reform(move |e: MouseEvent| { e.prevent_default(); i } ) } class="notification-close">{"󰅙"}</span>
                                            <div onclick={ let path = n.path.clone(); let disregard = disregard.clone(); navigate_to.reform(move |e: MouseEvent| { e.prevent_default(); disregard.emit(i); path.clone() } ) } class="notification-title">{&n.title}</div>
                                        </div>
                                        <div onclick={ let path = n.path.clone(); let disregard = disregard.clone(); navigate_to.reform(move |e: MouseEvent| { e.prevent_default(); disregard.emit(i); path.clone() } ) } >
                                            <div class="notification-body">
                                                if let Some(icon) = n.icon.as_ref() {
                                                    <img class="notification-icon" src={icon.clone()} />
                                                }
                                                <div class="notification-content">
                                                    <SpoilableText content={n.body.clone()} />
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                }
                            })
                        }
                    </div>
                }
            </div>
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct NotificationInfo {
    pub path: Path,
    pub title: String,
    pub body: String,
    pub icon: Option<String>,
}

impl From<SafePost> for NotificationInfo {
    fn from(post: SafePost) -> Self {
        return Self {
            // path: format!(
            //     "/{}/thread/{}",
            //     post.board_discriminator, post.thread_post_number
            // ),
            path: Path::Thread(post.board_discriminator.clone(), post.thread_post_number),
            title: "New post!".to_owned(),
            body: {
                let content = post.content.trim();
                let lim = 30;
                if content.len() > lim {
                    format!("{}...", &content[..(lim - 3)])
                } else {
                    content.to_owned()
                }
            },
            icon: post.file.map(|f| f.thumbnail),
        };
    }
}

#[allow(dead_code)]
#[derive(Clone, PartialEq, Debug)]
pub enum Path {
    Thread(String, i64),
    Board(String),
    Arbitrary(String),
}
