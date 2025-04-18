use std::{
    fmt::{Display, Formatter},
    sync::Arc,
};

use anyhow::anyhow;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BoardWithThreads {
    pub info: SafeBoard,
    pub threads: Vec<ThreadWithLazyPosts>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SafeBoard {
    pub name: String,
    pub discriminator: String,
    pub private: bool,
}

impl From<BoardWithThreads> for SafeBoard {
    fn from(b: BoardWithThreads) -> Self {
        // Self {
        //     name: b.name,
        //     discriminator: b.discriminator,
        //     private: b.private,
        // }
        b.info
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThreadWithPosts {
    pub board: i64,
    pub thread_post: SafePost,
    pub post_count: i64,
    pub posts: Vec<SafePost>,
    pub topic: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThreadWithLazyPosts {
    pub board: i64,
    pub thread_post: SafePost,
    pub post_count: i64,
    pub posts: Vec<SafePost>,
    pub topic: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SafePost {
    pub post_number: i64,
    pub file: Option<FileInfo>,
    pub thread_post_number: i64,
    pub board_discriminator: String,
    pub author: User,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub replies: Vec<Reply>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum User {
    Anonymous,
    Named(String),
    Mod(String),
}

impl User {
    pub fn load_from(s: Option<String>, moderator: bool) -> Self {
        match s {
            Some(s) => {
                if moderator {
                    Self::Mod(s)
                } else {
                    Self::Named(s)
                }
            }
            None => Self::Anonymous,
        }
    }
}

impl Display for User {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Anonymous => write!(f, "Anonymous"),
            Self::Named(s) => write!(f, "{}", s),
            Self::Mod(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileInfo {
    pub claimed: ClaimedFileInfo,
    pub board: MicroBoardInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClaimedFileInfo {
    pub path: String,
    pub thumbnail: String,
    pub hash: String,
    pub spoiler: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MicroBoardInfo {
    pub discriminator: String,
    pub private: bool,
    #[serde(skip)]
    pub id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreateBoard {
    pub discriminator: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreatePost {
    pub file: Option<CreateFile>,
    pub content: String,
    pub author: Option<String>,
    pub moderator: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreateFile {
    pub id: String,
    pub spoiler: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreateThread {
    pub post: CreatePost,
    pub topic: String,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Hash)]
pub struct Reply {
    pub post_number: String,
    pub thread_post_number: Option<String>,
    pub board_discriminator: String,
    pub external: bool,
}

impl Reply {
    pub fn from_str(s: &str, board: &str, thread: &str) -> anyhow::Result<Self> {
        let s = s.trim();

        let mut split = s.split('/');
        let first = split.next();
        let second = split.next();
        let third = split.next();
        let fourth = split.next();

        match (first, second, third, fourth) {
            (Some(">>>"), Some(b), Some(n), None) => {
                let board_discriminator = b.to_owned();
                let post_number = n.parse::<i64>()?.to_string();
                Ok(Reply {
                    post_number,
                    thread_post_number: None,
                    board_discriminator,
                    external: true,
                })
            }
            _ => {
                let mut split = s.split(">>");
                let first = split.next();
                let second = split.next();
                let third = split.next();

                match (first, second, third) {
                    (Some(""), Some(n), None) => {
                        let post_number = n.parse::<i64>()?.to_string();
                        Ok(Reply {
                            post_number,
                            thread_post_number: Some(thread.to_owned()),
                            board_discriminator: board.to_owned(),
                            external: false,
                        })
                    }
                    _ => Err(anyhow!("Invalid reply format")),
                }
            }
        }
    }
    pub fn text(&self, this_thread_post_number: String) -> String {
        if self.external {
            format!(
                ">>>/{board_discriminator}/{post_number}",
                board_discriminator = self.board_discriminator,
                post_number = self.post_number
            )
        } else {
            format!(
                ">>{}{}",
                self.post_number,
                if this_thread_post_number == self.post_number {
                    " (OP)"
                } else {
                    ""
                }
            )
        }
    }
    pub fn same_board_reply_text(&self) -> String {
        format!(">>{}", self.post_number)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Banner {
    pub path: String,
    pub href: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PushMessage {
    Open,
    NewPost(Arc<SafePost>),
    Close,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubscriptionData {
    pub endpoint: String,
    pub keys: SubscriptionKeys,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubscriptionKeys {
    pub auth: String,
    pub p256dh: String,
}
