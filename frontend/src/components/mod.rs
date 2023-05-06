mod board_name;
mod board_select;
mod board_title;
mod context_error;
mod editors;
mod file;
mod header;
mod maybe_link;
mod post;
mod post_box;
mod reply;
mod richtext;
mod settings;
mod spinner;
mod theme_editor;
mod thread;

pub use board_name::{BoardName, BoardNameType};
pub use board_select::BoardSelectBar;
pub use board_title::BoardTitle;
pub use context_error::ContextError;
pub use editors::*;
pub use file::File;
pub use header::Header;
pub use maybe_link::{MaybeLink, MaybeLinkProps};
pub use post::Post;
pub use post_box::PostBox;
pub use reply::Reply;
pub use richtext::RichText;
pub use settings::SettingsButton;
pub use spinner::Spinner;
pub use theme_editor::ThemeEditor;
pub use thread::Thread;

#[derive(Clone, PartialEq, Debug)]
pub enum HoveredOrExpandedState {
    None,
    Hovered,
    // Hovered{
    //     x: i32,
    //     y: i32,
    //     screen_y: i32,
    // },
    Expanded,
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