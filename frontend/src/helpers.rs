use common::structs::{Reply, SafePost};
use web_sys::HtmlInputElement;
use yew::prelude::*;

pub fn on_input_to_string(event: InputEvent) -> Option<HtmlInputElement> {
    use wasm_bindgen::JsCast;
    event
        .target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
}

pub fn on_change_to_string(event: Event) -> Option<HtmlInputElement> {
    use wasm_bindgen::JsCast;
    event
        .target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
}

pub fn on_change_textarea_to_string(event: Event) -> Option<web_sys::HtmlTextAreaElement> {
    use wasm_bindgen::JsCast;
    event
        .target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
}

pub fn on_input_textarea_to_string(event: InputEvent) -> Option<web_sys::HtmlTextAreaElement> {
    use wasm_bindgen::JsCast;
    event
        .target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
}

pub fn on_change_select_element(event: Event) -> Option<web_sys::HtmlSelectElement> {
    use wasm_bindgen::JsCast;
    event
        .target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok())
}

#[derive(Clone, PartialEq, Debug)]
pub struct CallbackEmitterContext {
    pub callback: Callback<Callback<Reply>>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct CallbackContext {
    pub callback: Callback<Reply>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct SuccessfulPostContext {
    pub callback: Callback<SafePost>,
}

/// Whether posting should also watch the thread. Off unless the member turned it on: replies
/// to their posts reach them without watching anything, and a list of watched threads is, for
/// someone who watches what they post in, a list of where they posted.
pub fn auto_watch_enabled() -> bool {
    use gloo::storage::{LocalStorage, Storage};
    LocalStorage::get::<bool>(AUTO_WATCH_KEY).unwrap_or(false)
}

pub const AUTO_WATCH_KEY: &str = "auto_watch";

/// Whether to notify in the page when someone replies to one of the member's posts. On unless
/// the member turned it off.
pub fn reply_notifications_enabled() -> bool {
    use gloo::storage::{LocalStorage, Storage};
    LocalStorage::get::<bool>(REPLY_NOTIFICATIONS_KEY).unwrap_or(true)
}

pub const REPLY_NOTIFICATIONS_KEY: &str = "reply_notifications";

/// The start of `content` for a notification preview: at most `max` characters, cut on a
/// character boundary (slicing bytes panics on multibyte text), marked when shortened.
pub fn preview(content: &str, max: usize) -> String {
    let content = content.trim();
    if content.chars().count() <= max {
        return content.to_owned();
    }
    let head: String = content.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", head.trim_end())
}

/// A post time the way `Post` shows it: in the member's timezone when they have one.
pub fn format_timestamp(
    timestamp: &chrono::DateTime<chrono::Utc>,
    timezone: Option<&chrono_tz::Tz>,
) -> String {
    let text = match timezone {
        Some(tz) => timestamp
            .with_timezone(tz)
            .format(env!("TIMESTAMP_FORMAT"))
            .to_string(),
        None => timestamp.format(env!("TIMESTAMP_FORMAT")).to_string(),
    };
    text.replace("  ", " ").replace(" at 0", " at ")
}

/// A compact post time for places with little room: "Oct 18, 11:39 PM".
pub fn format_short_timestamp(
    timestamp: &chrono::DateTime<chrono::Utc>,
    timezone: Option<&chrono_tz::Tz>,
) -> String {
    const SHORT: &str = "%b %-d, %-I:%M %p";
    match timezone {
        Some(tz) => timestamp.with_timezone(tz).format(SHORT).to_string(),
        None => timestamp.format(SHORT).to_string(),
    }
}

/// The posts a post replies to, as (board, post number), in the order they appear.
///
/// Reads the same `>>12` and `>>>/board/12` references the server does when it records
/// replies.
pub fn reply_targets(content: &str, board: &str, thread: i64) -> Vec<(String, i64)> {
    let thread = thread.to_string();
    let mut out: Vec<(String, i64)> = Vec::new();
    for token in content.split_whitespace() {
        let Ok(reply) = common::structs::Reply::from_str(token, board, &thread) else {
            continue;
        };
        let Ok(number) = reply.post_number.parse::<i64>() else {
            continue;
        };
        let target = (reply.board_discriminator, number);
        if !out.contains(&target) {
            out.push(target);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{preview, reply_targets};

    #[test]
    fn finds_what_a_post_replies_to() {
        assert_eq!(
            reply_targets(">>12 agreed, and >>>/other/7 too\n>>12 again", "b", 3),
            vec![("b".to_owned(), 12), ("other".to_owned(), 7)]
        );
        assert!(reply_targets("no replies here, only >> and >>x", "b", 3).is_empty());
        assert!(reply_targets("", "b", 3).is_empty());
    }


    #[test]
    fn previews_cut_on_characters_not_bytes() {
        assert_eq!(preview("short", 10), "short");
        assert_eq!(preview("  padded  ", 10), "padded");
        assert_eq!(preview(&"日".repeat(40), 5), format!("{}…", "日".repeat(4)));
        assert_eq!(preview(&"😀".repeat(31), 30), format!("{}…", "😀".repeat(29)));
        assert_eq!(preview("exactly ten", 11), "exactly ten");
    }
}
