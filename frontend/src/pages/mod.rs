mod board;
mod home;
mod join_status;
mod moderation;
mod not_found;
mod redeem;
mod request_access;
mod settings;
mod thread;

pub use board::BoardPage;
pub use home::Home;
pub use join_status::JoinStatus;
pub use moderation::Moderation;
pub use not_found::NotFound;
pub use redeem::Redeem;
pub use request_access::RequestAccess;
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
