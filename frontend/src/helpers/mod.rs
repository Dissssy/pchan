pub mod board_title;
pub mod lazy_post;
pub mod new_post_box;
pub mod post_container;
pub mod startswith_class;
pub mod thread_view;

#[derive(Clone, PartialEq, Debug)]
pub enum HoveredOrExpandedState {
    None,
    Hovered,
    Expanded,
}
