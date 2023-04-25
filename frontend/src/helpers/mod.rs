pub mod banner;
pub mod board_link;
pub mod board_list;
pub mod board_title;
pub mod boards_navbar;
pub mod delete_button;
pub mod lazy_post;
pub mod new_post_box;
pub mod possibly_long_text;
pub mod post_container;
pub mod rich_post_content;
pub mod startswith_class;
pub mod thread_view;

#[derive(Clone, PartialEq, Debug)]
pub enum HoveredOrExpandedState {
    None,
    Hovered,
    Expanded,
}
