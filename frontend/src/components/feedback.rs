use serde::{Deserialize, Serialize};
use yew::prelude::*;

// const FEEDBACK_URL: &str = env!("FEEDBACK_WEBHOOK_URL");

#[function_component]
pub fn FeedbackButton() -> Html {
    // allows the user to submit feedback and collects information about the user's browser as well as the current page and api cache state

    // let expanded = use_state(|| false);

    // if *expanded {
    //     html! {
    //         <>
    //             <div class="popup-blur" id="ignore-transition" onclick={ let expanded = expanded.clone(); Callback::from(move |_e: MouseEvent| expanded.set(!*expanded) )}/>
    //             <div class="popup" id="ignore-transition">
    //                 <div class="valign">
    //                     <div class="halign">
    //                         <FeedbackForm />
    //                         <div class="feedback-box-button" onclick={ let expanded = expanded.clone(); Callback::from(move |_e: MouseEvent| expanded.set(!*expanded) ) }>{"ℹ️"}</div>
    //                     </div>
    //                 </div>
    //             </div>
    //         </>
    //     }
    // } else {
    //     html! {
    //         <div class="feedback-box-button" onclick={ let expanded = expanded.clone(); Callback::from(move |_e: MouseEvent| expanded.set(!*expanded) ) }>{"ℹ️"}</div>
    //     }
    // }
    html! {}
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct DiscordWebhookBody {
    content: String,
    username: String,
    avatar_url: String,
    embeds: Vec<DiscordEmbed>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct DiscordEmbed {
    title: String,
    description: String,
    url: String,
    color: u32,
    fields: Vec<DiscordEmbedField>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct DiscordEmbedField {
    name: String,
    value: String,
    inline: bool,
}

#[function_component]
pub fn FeedbackForm() -> Html {
    html! {
        <div class="feedback-box">
            <h1>{"Feedback"}</h1>
            <p>{"Please use this form to submit feedback about the site. If you are reporting a bug, please include as much information as possible, including the steps to reproduce the bug. This form WILL collect some information about your browser as well as the current page and cache."}</p>
        </div>
    }
}
