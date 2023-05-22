mod banner_ad;
mod board_name;
mod board_select;
mod board_title;
mod context_error;
mod delete_button;
mod editors;
// mod feedback;
mod file;
mod footer;
mod header;
mod maybe_link;
mod notifications;
mod post;
mod post_box;
mod reply;
mod richtext;
mod settings;
mod spinner;
mod thread;
mod watch_button;

pub use banner_ad::BannerAd;
pub use board_name::{BoardName, BoardNameType};
pub use board_select::BoardSelectBar;
pub use board_title::BoardTitle;
pub use context_error::ContextError;
pub use delete_button::DeleteButton;
pub use editors::*;
// pub use feedback::FeedbackButton;
pub use editors::theme_editor::ThemeEditor;
pub use editors::timezone_editor::TimezoneEditor;
pub use file::File;
pub use footer::Footer;
pub use header::Header;
pub use maybe_link::{MaybeLink, MaybeLinkProps};
pub use notifications::NotificationBox;
pub use post::Post;
pub use post_box::PostBox;
pub use reply::Reply;
pub use richtext::RichText;
pub use settings::SettingsButton;
pub use spinner::Spinner;
pub use thread::Thread;
pub use watch_button::WatchButton;
use yew::AttrValue;

#[derive(Clone, PartialEq, Debug)]
pub enum HoveredOrExpandedState {
    None,
    Hovered { x: i32, y: i32, offset: OffsetType },
    // Hovered{
    //     x: i32,
    //     y: i32,
    //     screen_y: i32,
    // },
    Expanded { x: i32, y: i32, offset: OffsetType },
}

#[derive(Clone, PartialEq, Debug, Copy)]
pub enum OffsetType {
    Top,
    Center,
    Bottom,
}

impl OffsetType {
    pub fn percent(&self) -> AttrValue {
        match self {
            Self::Top => "-100%",
            Self::Center => "-50%",
            Self::Bottom => "0%",
        }
        .into()
    }
}

#[derive(Clone, PartialEq, Default, Copy)]
pub struct ParentOffset {
    pub x: i32,
    pub y: i32,
}

// impl HoveredOrExpandedState {
//     pub fn get_style(&self) -> String {
//         if let HoveredOrExpandedState::Hovered { x, y, screen_y } = self {
//             format!("
//                 :root {{
//                     --floating-image-x: {}px;
//                     --floating-image-y: {}px;
//                     --floating-image-screen-y: {}px;
//                 }}
//                 ", x, y, screen_y)
//         } else {
//             "".to_owned()
//         }
//     }
// }
