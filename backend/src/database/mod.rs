use std::io::{Read, Write};

use anyhow::Result;
use deadpool::managed::Object;
use diesel::dsl::now;
use diesel::insert_into;
use diesel::query_dsl::methods::FilterDsl;
use diesel::query_dsl::methods::OrderDsl;
use diesel::ExpressionMethods;
use diesel_async::RunQueryDsl;
use diesel_async::{pooled_connection::AsyncDieselConnectionManager, AsyncPgConnection};

pub struct Users {
    valid_users: Vec<String>,
}

impl Users {
    pub fn new() -> Result<Self> {
        Ok(Self {
            valid_users: vec![],
        })
    }
    pub async fn open(&mut self) -> Result<()> {
        if let Ok(auth) = tokio::fs::read("./auth.bin.gz").await {
            let mut bytes = vec![];
            {
                let mut auth = flate2::read::GzDecoder::new(auth.as_slice());
                let _ = auth.read_to_end(&mut bytes);
            }
            let auth: Vec<String> = postcard::from_bytes(&bytes).unwrap_or_default();
            self.valid_users = auth;
        }

        Ok(())
    }
    pub async fn close(&mut self) -> Result<()> {
        let mut auth = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        auth.write_all(&postcard::to_allocvec(&self.valid_users)?)?;
        tokio::fs::write("./auth.bin.gz", auth.finish()?).await?;
        Ok(())
    }

    pub async fn is_auth(&mut self, token: String) -> Result<bool> {
        Ok(self.valid_users.contains(&token))
    }
    pub async fn add_auth(&mut self, token: String) -> Result<()> {
        self.valid_users.push(token);
        Ok(())
    }
    pub async fn sync_auth(&mut self, tokens: Vec<String>) -> Result<()> {
        self.valid_users = tokens;
        Ok(())
    }
    pub async fn remove_auth(&mut self, token: String) -> Result<()> {
        self.valid_users.retain(|x| x != &token);
        Ok(())
    }
}

pub struct Database;

impl Database {
    pub async fn create_board(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        new_discriminator: String,
        new_name: String,
    ) -> Result<()> {
        use crate::schema::boards::dsl::*;

        insert_into(boards)
            .values((discriminator.eq(new_discriminator), name.eq(new_name)))
            .execute(conn)
            .await?;
        Ok(())
    }
    pub async fn get_boards(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> Result<Vec<crate::schema::Board>> {
        use crate::schema::boards::dsl::*;

        let results = boards
            .order(name.desc())
            .load::<crate::schema::Board>(&mut *conn)
            .await?;
        Ok(results)
    }

    pub async fn get_board(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discim: String,
    ) -> Result<crate::schema::Board> {
        use crate::schema::boards::dsl::*;

        let results = boards
            .filter(discriminator.eq(discim))
            .first::<crate::schema::Board>(&mut *conn)
            .await?;
        Ok(results)
    }

    pub async fn get_post(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discrim: String,
        number: i64,
    ) -> Result<crate::schema::SafePost> {
        use crate::schema::posts::dsl::*;

        let this_board = Self::get_board(conn, discrim).await?;
        let results = posts
            .filter(board.eq(this_board.id))
            .filter(post_number.eq(number))
            .first::<crate::schema::Post>(&mut *conn)
            .await?;
        results.safe(conn).await
    }

    pub async fn get_thread(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discrim: String,
        number: i64,
    ) -> Result<crate::schema::ThreadWithPosts> {
        use crate::schema::threads::dsl::*;
        let this_board = Self::get_board(conn, discrim.clone()).await?;
        let this_post = Self::get_post(conn, discrim, number).await?;
        let results = threads
            .filter(board.eq(this_board.id))
            .filter(post_id.eq(this_post.id))
            .first::<crate::schema::Thread>(&mut *conn)
            .await?;
        results.with_posts(conn).await
    }

    pub async fn get_thread_from_post_number(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        bboard: i64,
        number: i64,
    ) -> Result<crate::schema::ThreadWithPosts> {
        use crate::schema::threads::dsl::*;
        let this_post = Self::get_post_from_post_number(conn, bboard, number).await?;
        let results = threads
            .filter(board.eq(bboard))
            .filter(post_id.eq(this_post.id))
            .first::<crate::schema::Thread>(&mut *conn)
            .await?;
        results.with_posts(conn).await
    }

    pub async fn get_post_from_post_number(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        bboard: i64,
        number: i64,
    ) -> Result<crate::schema::SafePost> {
        use crate::schema::posts::dsl::*;
        let results = posts
            .filter(board.eq(bboard))
            .filter(post_number.eq(number))
            .first::<crate::schema::Post>(&mut *conn)
            .await?;
        results.safe(conn).await
    }

    pub async fn create_thread(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        tboard: String,
        threadinfo: crate::schema::CreateThread,
        actual_author: String,
    ) -> Result<crate::schema::ThreadWithPosts> {
        use crate::schema::threads::dsl::*;

        let this_board = Self::get_board(conn, tboard.clone()).await?;
        let mut t = insert_into(threads)
            .values((board.eq(this_board.id), post_id.eq(0), latest_post.eq(0)))
            .get_result::<crate::schema::Thread>(conn)
            .await?;

        let p = Self::create_post(
            conn,
            this_board.id,
            tboard,
            t.id,
            threadinfo.post,
            actual_author,
        )
        .await?;
        t.post_id = p.id;
        t.with_posts(conn).await
    }

    pub async fn create_post(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        tboard: i64,
        discrim: String,
        tthread: i64,
        post: crate::schema::CreatePost,
        tactual_author: String,
    ) -> Result<crate::schema::SafePost> {
        use crate::schema::posts::dsl::*;
        // attempt to parse replies from the post, these are in the form of ">>{post_number}" or ">>/{board}/{post_number}"
        let replies = post
            .content
            .split_whitespace()
            .filter(|x| x.starts_with(">>"))
            .map(|x| {
                if x.contains('/') {
                    let mut split = x.split('/');
                    split.next();
                    let bboard = split.next().unwrap_or_default();
                    let post = split.next().unwrap_or_default();
                    (Some(bboard.to_string()), post.parse::<i64>().ok())
                } else {
                    let post = x.trim_start_matches(">>");
                    (None, post.parse::<i64>().ok())
                }
            })
            .collect::<Vec<(Option<String>, Option<i64>)>>()
            .iter()
            .flat_map(|(bboard, post)| {
                if let Some(post) = post {
                    if let Some(bboard) = bboard {
                        if !bboard.is_empty() {
                            Some((bboard.to_string(), *post))
                        } else {
                            None
                        }
                    } else {
                        Some((discrim.clone(), *post))
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<(String, i64)>>();

        let mut replieses = Vec::new();

        for (bboard, post) in replies {
            let this_post = Self::get_post(conn, bboard, post).await.map(|x| x.id);
            if let Ok(this_post) = this_post {
                replieses.push(this_post);
            }
        }
        let t = insert_into(posts).values((
            post_number.eq(Self::get_board(conn, discrim.clone()).await?.post_count + 1),
            image.eq(post.image),
            thread.eq(tthread),
            board.eq(tboard),
            author.eq(post.author.clone()),
            content.eq(post.content.clone()),
            replies_to.eq(replieses),
            timestamp.eq(now),
            actual_author.eq(tactual_author),
        ));
        println!("{:?}", diesel::debug_query(&t));
        let t = t.get_result::<crate::schema::Post>(conn).await?;

        t.safe(conn).await
    }
}

// CREATE OR REPLACE FUNCTION update_thread()
// RETURNS TRIGGER AS $$
// BEGIN
//     IF (SELECT COUNT(*) FROM posts WHERE thread = NEW.thread) <= 300 THEN
//         UPDATE threads SET latest_post = NEW.id WHERE id = NEW.thread;
//     END IF;
//     IF (SELECT COUNT(*) FROM posts WHERE thread = NEW.thread) = 0 THEN
//         UPDATE threads SET post_id = NEW.id WHERE id = NEW.thread;
//     END IF;
//     RETURN NEW;
// END;
// $$ LANGUAGE plpgsql;

// CREATE OR REPLACE FUNCTION update_board()
// RETURNS TRIGGER AS $$
// BEGIN
//     UPDATE boards SET post_count = NEW.post_count WHERE id = NEW.board;
//     RETURN NEW;
// END;
// $$ LANGUAGE plpgsql;
