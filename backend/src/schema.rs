use anyhow::Result;
use deadpool::managed::Object;
use diesel::query_dsl::methods::FilterDsl;
use diesel::query_dsl::methods::LimitDsl;
use diesel::query_dsl::methods::OrderDsl;
use diesel::ExpressionMethods;
use diesel::Queryable;
use diesel_async::RunQueryDsl;
use diesel_async::{pooled_connection::AsyncDieselConnectionManager, AsyncPgConnection};
use serde::{Deserialize, Serialize};

diesel::table! {
    boards (id) {
        id -> BigInt,
        name -> Text,
        discriminator -> Text,
    }
}

#[derive(Queryable, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Board {
    pub id: i64,
    pub name: String,
    pub discriminator: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BoardWithThreads {
    pub id: i64,
    pub name: String,
    pub discriminator: String,
    pub threads: Vec<ThreadWithLazyPosts>,
}

impl Board {
    pub async fn with_threads(
        &self,
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> Result<BoardWithThreads> {
        use crate::schema::threads::dsl::*;
        let mut bthreads = Vec::new();
        for thread in threads
            .filter(board.eq(self.id))
            .load::<Thread>(conn)
            .await?
            .iter()
        {
            bthreads.push(thread.with_lazy_posts(conn).await?);
        }

        Ok(BoardWithThreads {
            id: self.id,
            name: self.name.clone(),
            discriminator: self.discriminator.clone(),
            threads: bthreads,
        })
    }
}

diesel::table! {
    threads (id) {
        id -> BigInt,
        board -> BigInt,
        post_id -> BigInt,
        latest_post -> BigInt,
    }
}

#[derive(Queryable, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Thread {
    pub id: i64,
    pub board: i64,
    pub post_id: i64,
    pub latest_post: i64,
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

impl Thread {
    pub async fn with_posts(
        &self,
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> Result<ThreadWithPosts> {
        use crate::schema::posts::dsl::*;
        let tposts = posts
            .filter(thread.eq(self.id))
            .filter(id.ne(self.post_id))
            .load::<Post>(conn)
            .await?
            .iter()
            .map(|p| p.safe())
            .collect();
        let tpost = posts
            .filter(id.eq(self.post_id))
            .first::<Post>(conn)
            .await?;
        Ok(ThreadWithPosts {
            id: self.id,
            board: self.board,
            thread_post: tpost.safe(),
            posts: tposts,
        })
    }
    pub async fn with_lazy_posts(
        &self,
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> Result<ThreadWithLazyPosts> {
        use crate::schema::posts::dsl::*;
        let tposts = posts
            .filter(thread.eq(self.id))
            .filter(id.ne(self.post_id))
            .order(post_number.asc())
            .limit(5)
            .load::<Post>(conn)
            .await?
            .iter()
            .map(|p| p.safe())
            .collect();
        let tpost = posts
            .filter(id.eq(self.post_id))
            .first::<Post>(conn)
            .await?;
        Ok(ThreadWithLazyPosts {
            id: self.id,
            board: self.board,
            thread_post: tpost.safe(),
            posts: tposts,
        })
    }
}

diesel::table! {
    posts (id) {
        id -> BigInt,
        post_number -> BigInt,
        image -> Nullable<Text>,
        thread -> BigInt,
        board -> BigInt,
        author -> Nullable<Text>,
        actual_author -> Text,
        content -> Text,
        timestamp -> Timestamp,
        replies_to -> Array<BigInt>,
    }
}

#[derive(Queryable, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Post {
    pub id: i64,
    pub post_number: i64,
    pub image: Option<String>,
    pub thread: i64,
    pub board: i64,
    pub author: Option<String>,
    pub actual_author: String,
    pub content: String,
    pub timestamp: chrono::NaiveDateTime,
    pub replies_to: Vec<i64>,
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
    pub timestamp: chrono::NaiveDateTime,
    pub replies_to: Vec<i64>,
}

impl Post {
    pub fn safe(&self) -> SafePost {
        SafePost {
            id: self.id,
            post_number: self.post_number,
            image: self.image.clone(),
            thread: self.thread,
            board: self.board,
            author: self.author.clone(),
            content: self.content.clone(),
            timestamp: self.timestamp,
            replies_to: self.replies_to.clone(),
        }
    }
}
