mod board;
mod home;
mod settings;
mod thread;
mod not_found;

pub use board::BoardPage;
pub use home::Home;
pub use settings::Settings;
pub use thread::ThreadPage;
pub use not_found::NotFound;

// #[derive(Clone, PartialEq, Debug)]
// pub struct BoardContext {
//     pub discriminator: String,
// }

// #[derive(Clone, PartialEq, Debug)]
// pub struct ThreadContext {
//     pub thread_id: String,
// }
