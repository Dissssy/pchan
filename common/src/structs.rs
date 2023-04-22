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
    pub posts: Vec<SafePost>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThreadWithLazyPosts {
    pub id: i64,
    pub board: i64,
    pub thread_post: SafePost,
    pub posts: Vec<SafePost>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SafePost {
    pub id: i64,
    pub post_number: i64,
    pub image: Option<String>,
    pub thread: i64,
    pub board: i64,
    pub author: Option<String>,
    pub content: String,
    pub timestamp: String,
    pub replies: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreateBoard {
    pub discriminator: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreatePost {
    pub image: Option<String>,
    pub content: String,
    pub author: Option<String>,
}
