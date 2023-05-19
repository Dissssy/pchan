mod board;
mod home;
mod not_found;
mod settings;
mod thread;

pub use board::BoardPage;
pub use home::Home;
pub use not_found::NotFound;
pub use settings::Settings;
pub use thread::ThreadPage;

// #[derive(Clone, PartialEq, Debug)]
// pub struct BoardContext {
//     pub discriminator: String,
// }

// #[derive(Clone, PartialEq, Debug)]
// pub struct ThreadContext {
//     pub thread_id: String,
// }
