use anyhow::Result;
use common::structs::*;
use deadpool::managed::Object;
use diesel::dsl::count;
use diesel::query_dsl::methods::SelectDsl;
use diesel::{
    query_dsl::methods::{FilterDsl, LimitDsl, OrderDsl},
    ExpressionMethods, Queryable,
};
use diesel_async::RunQueryDsl;
use diesel_async::{pooled_connection::AsyncDieselConnectionManager, AsyncPgConnection};

diesel::table! {
    boards (id) {
        id -> BigInt,
        name -> Text,
        discriminator -> Text,
        post_count -> BigInt,
    }
}

#[derive(Queryable, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Board {
    pub id: i64,
    pub name: String,
    pub discriminator: String,
    pub post_count: i64,
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
            .order(latest_post.desc())
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

    pub fn safe(&self) -> SafeBoard {
        SafeBoard {
            name: self.name.clone(),
            discriminator: self.discriminator.clone(),
        }
    }
}

diesel::table! {
    threads (id) {
        id -> BigInt,
        board -> BigInt,
        post_id -> BigInt,
        latest_post -> BigInt,
        topic -> Text,
    }
}

#[derive(Queryable, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Thread {
    pub id: i64,
    pub board: i64,
    pub post_id: i64,
    pub latest_post: i64,
    pub topic: String,
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
            .order(post_number.asc())
            .load::<Post>(conn)
            .await?;
        let mut safeposts = Vec::new();
        for post in tposts.iter() {
            safeposts.push(post.safe(conn).await?);
        }
        let post_count = posts
            .select(count(id))
            .filter(thread.eq(self.id))
            .filter(id.ne(self.post_id))
            .first::<i64>(conn)
            .await?;
        let tpost = posts
            .filter(id.eq(self.post_id))
            .first::<Post>(conn)
            .await?;
        Ok(ThreadWithPosts {
            id: self.id,
            board: self.board,
            post_count,
            topic: self.topic.clone(),
            thread_post: tpost.safe(conn).await?,
            posts: safeposts,
        })
    }
    pub async fn with_lazy_posts(
        &self,
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> Result<ThreadWithLazyPosts> {
        use crate::schema::posts::dsl::*;
        let tposts: Vec<Post> = posts
            .filter(thread.eq(self.id))
            .filter(id.ne(self.post_id))
            .order(post_number.desc())
            .limit(5)
            .load::<Post>(conn)
            .await?;
        let post_count = posts
            .select(count(id))
            .filter(thread.eq(self.id))
            .filter(id.ne(self.post_id))
            .first::<i64>(conn)
            .await?;
        let mut safeposts = Vec::new();
        for post in tposts.iter() {
            safeposts.push(post.safe(conn).await?);
        }
        safeposts.reverse();
        let tpost = posts
            .filter(id.eq(self.post_id))
            .first::<Post>(conn)
            .await?;
        Ok(ThreadWithLazyPosts {
            id: self.id,
            board: self.board,
            post_count,
            topic: self.topic.clone(),
            thread_post: tpost.safe(conn).await?,
            posts: safeposts,
        })
    }
}

diesel::table! {
    posts (id) {
        id -> BigInt,
        post_number -> BigInt,
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
    pub thread: i64,
    pub board: i64,
    pub author: Option<String>,
    pub actual_author: String,
    pub content: String,
    pub timestamp: chrono::NaiveDateTime,
    pub replies_to: Vec<i64>,
}

impl Post {
    pub async fn safe(
        &self,
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> Result<SafePost> {
        use crate::schema::posts::dsl::*;
        let replies = posts
            .load::<Post>(conn)
            .await?
            .iter()
            .flat_map(|p| {
                if p.replies_to.contains(&self.id) {
                    Some(p.id)
                } else {
                    None
                }
            })
            .collect::<Vec<i64>>();

        let mut newreplies = Vec::new();
        for reply in replies {
            newreplies.push(get_reply_info(reply, self.board, conn).await?);
        }

        Ok(SafePost {
            id: self.id,
            post_number: self.post_number,
            file: get_file(conn, self.id).await?,
            thread: thread_post_number(self.thread, conn).await?,
            board: self.board,
            author: self.author.clone(),
            content: self.content.clone(),
            timestamp: format!("{}", self.timestamp),
            replies: newreplies,
        })
    }
}

pub async fn get_reply_info(
    tid: i64,
    og_board: i64,
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
) -> Result<Reply> {
    use crate::schema::posts::dsl::*;
    let post = posts.filter(id.eq(tid)).first::<Post>(conn).await?;
    let external = post.board != og_board;

    let thisboard = get_board_discrim(post.board, conn).await?;

    Ok(Reply {
        post_number: post.post_number,
        board_discriminator: thisboard,
        external,
    })
}

pub async fn get_board_discrim(
    board: i64,
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
) -> Result<String> {
    use crate::schema::boards::dsl::*;
    let board = boards.filter(id.eq(board)).first::<Board>(conn).await?;
    Ok(board.discriminator)
}

pub async fn thread_post_number(
    thread: i64,
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
) -> Result<i64> {
    use crate::schema::threads::dsl::*;
    let thread = threads.filter(id.eq(thread)).first::<Thread>(conn).await?;
    let post = post_number(thread.post_id, conn).await?;
    Ok(post)
}

pub async fn post_number(
    post: i64,
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
) -> Result<i64> {
    use crate::schema::posts::dsl::*;
    let post = posts.filter(id.eq(post)).first::<Post>(conn).await?;
    Ok(post.post_number)
}

pub async fn get_file(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    tid: i64,
) -> Result<Option<FileInfo>> {
    use crate::schema::files::dsl::*;
    use diesel::result::OptionalExtension;
    let file = files
        .filter(id.eq(tid))
        .first::<File>(conn)
        .await
        .optional()?
        .map(|x| x.info());
    Ok(file)
}

diesel::table! {
    files (id) {
        id -> BigInt,
        filepath -> Text,
        hash -> Text,
    }
}

#[derive(Queryable, Debug, Clone, PartialEq, Eq, Hash)]
pub struct File {
    pub id: i64,
    pub filepath: String,
    pub hash: String,
}

impl File {
    pub fn info(&self) -> FileInfo {
        let thumbnail = format!("{}-thumb.jpg", self.filepath);
        FileInfo {
            path: self.filepath.clone(),
            thumbnail,
            hash: self.hash.clone(),
        }
    }
}

diesel::table! {
    banners (id) {
        id -> BigInt,
        img_path -> Text,
        href -> Nullable<Text>,
        boards -> Nullable<Array<BigInt>>,
    }
}

#[derive(Queryable, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Banner {
    pub id: i64,
    pub img_path: String,
    pub href: Option<String>,
    pub boards: Option<Vec<i64>>,
}

impl Banner {
    pub fn safe(&self) -> common::structs::Banner {
        common::structs::Banner {
            path: self.img_path.clone(),
            href: self.href.clone(),
        }
    }
}
