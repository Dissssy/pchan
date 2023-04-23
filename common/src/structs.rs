use anyhow::anyhow;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BoardWithThreads {
    pub id: i64,
    pub name: String,
    pub discriminator: String,
    pub threads: Vec<ThreadWithLazyPosts>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SafeBoard {
    pub name: String,
    pub discriminator: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThreadWithPosts {
    pub id: i64,
    pub board: i64,
    pub thread_post: SafePost,
    pub post_count: i64,
    pub posts: Vec<SafePost>,
    pub topic: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThreadWithLazyPosts {
    pub id: i64,
    pub board: i64,
    pub thread_post: SafePost,
    pub post_count: i64,
    pub posts: Vec<SafePost>,
    pub topic: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SafePost {
    pub id: i64,
    pub post_number: i64,
    pub file: Option<FileInfo>,
    pub thread: i64,
    pub board: i64,
    pub author: Option<String>,
    pub content: String,
    pub timestamp: String,
    pub replies: Vec<Reply>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub thumbnail: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreateBoard {
    pub discriminator: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreatePost {
    pub file: Option<String>,
    pub content: String,
    pub author: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreateThread {
    pub post: CreatePost,
    pub topic: String,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Hash)]
pub struct Reply {
    pub post_number: i64,
    pub board_discriminator: String,
    pub external: bool,
}

impl Reply {
    pub fn from_str(s: &str, board: &str) -> anyhow::Result<Self> {
        let s = s.trim();
        // >>{post_number}
        // or
        // >>>/{board_discriminator}/{post_number}

        // THIS BOARD POST DOES NOT HAVE A /

        let mut split = s.split('/');
        let first = split.next();
        let second = split.next();
        let third = split.next();
        let fourth = split.next();

        match (first, second, third, fourth) {
            (Some(">>>"), Some(b), Some(n), None) => {
                let board_discriminator = b.to_owned();
                let post_number = n.parse::<i64>()?;
                Ok(Reply {
                    post_number,
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
                        let post_number = n.parse::<i64>()?;
                        Ok(Reply {
                            post_number,
                            board_discriminator: board.to_owned(),
                            external: false,
                        })
                    }
                    _ => Err(anyhow!("Invalid reply format")),
                }
            }
        }
    }
    pub fn link(&self) -> String {
        format!(
            "/{board_discriminator}/thread/{post_number}",
            board_discriminator = self.board_discriminator,
            post_number = self.post_number
        )
    }
    pub fn text(&self) -> String {
        if self.external {
            format!(
                ">>>/{board_discriminator}/{post_number}",
                board_discriminator = self.board_discriminator,
                post_number = self.post_number
            )
        } else {
            format!(">>{post_number}", post_number = self.post_number)
        }
    }
}
