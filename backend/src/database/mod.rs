use std::io::{Read, Write};

use anyhow::{anyhow, Result};
use common::structs::*;
use deadpool::managed::Object;
use diesel::dsl::now;
use diesel::insert_into;
use diesel::query_dsl::methods::FilterDsl;
use diesel::query_dsl::methods::OrderDsl;
use diesel::ExpressionMethods;
use diesel::PgArrayExpressionMethods;
use diesel_async::RunQueryDsl;
use diesel_async::{pooled_connection::AsyncDieselConnectionManager, AsyncPgConnection};

use crate::schema::Banner;
use crate::schema::Post;

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
        discriminator: String,
        number: i64,
    ) -> Result<SafePost> {
        Self::get_raw_post(conn, discriminator, number)
            .await?
            .safe(conn)
            .await
    }

    // pub async fn get_post_from_id(
    //     conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    //     id: i64,
    // ) -> Result<SafePost> {
    //     use crate::schema::posts::dsl::*;

    //     let post = posts
    //         .filter(id.eq(id))
    //         .first::<crate::schema::Post>(&mut *conn)
    //         .await?;
    //     post.safe(conn).await
    // }

    async fn get_raw_post(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discriminator: String,
        number: i64,
    ) -> Result<Post> {
        use crate::schema::posts::dsl::*;

        let this_board = Self::get_board(conn, discriminator).await?;
        Ok(posts
            .filter(board.eq(this_board.id))
            .filter(post_number.eq(number))
            .first::<crate::schema::Post>(&mut *conn)
            .await?)
    }

    pub async fn delete_post(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discriminator: String,
        number: i64,
        token: String,
    ) -> Result<i64> {
        let tpost = Self::get_raw_post(conn, discriminator.clone(), number).await?;
        let tauthor =
            tpost.actual_author == common::hash_with_salt(&token, &format!("{}", tpost.id));
        let tadmin = Self::is_admin(conn, token, tpost.board).await?;

        if !(tadmin || tauthor) {
            return Err(anyhow!("Not authorized to delete post"));
        }

        let tthread = Self::get_thread(conn, discriminator, number).await.is_ok();
        let id = tpost.id;
        match (tadmin, tauthor, tthread, tpost) {
            // if the user is an admin they can delete a post
            (true, _, false, post) => {
                Self::raw_delete_post(conn, post.id).await?;
            }
            // if the user is an admin they can delete a thread
            (true, _, true, post) => {
                Self::raw_delete_thread(conn, post.id).await?;
            }
            // if the user is the author of the post and it is not a thread they can delete it
            (false, true, false, post) => {
                Self::raw_delete_post(conn, post.id).await?;
            }
            // otherwise they are not authorized to delete the post
            _ => {
                return Err(anyhow::anyhow!("Not authorized to delete post"));
            }
        }
        Ok(id)
    }

    async fn raw_delete_post(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        tid: i64,
    ) -> Result<()> {
        use crate::schema::posts::dsl::*;

        diesel::delete(posts.filter(id.eq(tid)))
            .execute(conn)
            .await?;
        Ok(())
    }

    async fn raw_delete_thread(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        tid: i64,
    ) -> Result<()> {
        use crate::schema::posts::dsl::*;

        diesel::delete(posts.filter(id.eq(tid)))
            .execute(conn)
            .await?;
        Ok(())
    }

    async fn is_admin(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: String,
        board_id: i64,
    ) -> Result<bool> {
        Ok(false)
    }

    pub async fn create_file(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        file: FileInfo,
        post_id: i64,
    ) -> Result<crate::schema::File> {
        use crate::schema::files::dsl::*;

        let tf = insert_into(files)
            .values((
                filepath.eq(file.path),
                hash.eq(file.hash),
                id.eq(post_id),
                spoiler.eq(file.spoiler),
            ))
            .get_result::<crate::schema::File>(conn)
            .await?;

        Ok(tf)
    }
    pub async fn get_thread(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discriminator: String,
        number: i64,
    ) -> Result<ThreadWithPosts> {
        use crate::schema::threads::dsl::*;
        let this_board = Self::get_board(conn, discriminator.clone()).await?;
        let this_post = Self::get_post(conn, discriminator, number).await?;
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
    ) -> Result<ThreadWithPosts> {
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
    ) -> Result<SafePost> {
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
        mut thread: CreateThread,
        actual_author: String,
    ) -> Result<ThreadWithPosts> {
        use crate::schema::threads::dsl::*;

        if thread.topic.is_empty() {
            return Err(anyhow::anyhow!("No topic provided"));
        }

        thread.topic = replace_possible_profanity(thread.topic);

        if thread.post.file.is_none() {
            return Err(anyhow::anyhow!("No file provided"));
        }

        let this_board = Self::get_board(conn, tboard.clone()).await?;
        let mut t = insert_into(threads)
            .values((
                board.eq(this_board.id),
                post_id.eq(0),
                latest_post.eq(0),
                topic.eq(thread.topic),
            ))
            .get_result::<crate::schema::Thread>(conn)
            .await?;

        let p = match Self::create_post(
            conn,
            this_board.id,
            tboard,
            t.id,
            thread.post,
            actual_author,
            None,
        )
        .await
        {
            Ok(p) => p,
            Err(e) => {
                diesel::delete(threads.filter(id.eq(t.id)))
                    .execute(conn)
                    .await?;
                return Err(e);
            }
        };
        t.post_id = p.id;
        t.with_posts(conn).await
    }

    pub async fn create_post(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        tboard: i64,
        discriminator: String,
        tthread: i64,
        mut post: CreatePost,
        tactual_author: String,
        check_hash_against: Option<Vec<FileInfo>>,
    ) -> Result<SafePost> {
        use crate::schema::posts::dsl::*;

        post.content = post.content.trim().to_owned();
        if post.content.is_empty() && post.file.is_none() {
            return Err(anyhow::anyhow!(
                "Either content or an file must be provided for any post"
            ));
        }

        post.content = replace_possible_profanity(post.content);
        post.author = post.author.map(replace_possible_profanity);

        let replies = post
            .content
            .split_whitespace()
            .flat_map(|x| common::structs::Reply::from_str(x, &discriminator))
            .collect::<Vec<common::structs::Reply>>();

        let mut replieses = Vec::new();

        for reply in replies {
            let this_post = Self::get_post(conn, reply.board_discriminator, reply.post_number)
                .await
                .map(|x| x.id);
            if let Ok(this_post) = this_post {
                replieses.push(this_post);
            }
        }

        let lock = crate::FS_LOCK.lock().await;

        let pending_file = if let Some(file) = post.file.clone() {
            let f = crate::UNCLAIMED_FILES
                .lock()
                .await
                .claim_file(&file, tactual_author.clone())
                .await?;

            if let Some(files_check) = check_hash_against {
                if files_check.iter().any(|x| x.hash == f.hash) {
                    return Err(anyhow::anyhow!("File already exists"));
                }
            }

            Some(f)
        } else {
            None
        };

        let t = insert_into(posts).values((
            post_number.eq(Self::get_board(conn, discriminator.clone())
                .await?
                .post_count
                + 1),
            thread.eq(tthread),
            board.eq(tboard),
            author.eq(post.author.clone()),
            content.eq(post.content.clone()),
            replies_to.eq(replieses),
            timestamp.eq(now),
            actual_author.eq(tactual_author.clone()),
        ));
        let p = t.get_result::<crate::schema::Post>(conn).await?;

        if let Some(f) = pending_file {
            crate::database::Database::create_file(conn, f, p.id).await?;
        }

        drop(lock);

        diesel::update(posts.filter(id.eq(p.id)))
            .set(actual_author.eq(common::hash_with_salt(
                &tactual_author,
                &format!("{}", p.id),
            )))
            .execute(conn)
            .await?;

        p.safe(conn).await
    }

    pub async fn get_all_files(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> Result<Vec<FileInfo>> {
        use crate::schema::files::dsl::*;
        let filelist = files
            .load::<crate::schema::File>(conn)
            .await?
            .iter()
            .map(|x| x.raw_info())
            .collect::<Vec<FileInfo>>();
        Ok(filelist)
    }

    pub async fn get_random_banner(
        board_discriminator: String,
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> Result<common::structs::Banner> {
        use crate::schema::banners::dsl::*;
        use diesel::query_dsl::methods::OrFilterDsl;
        let board = Self::get_board(conn, board_discriminator.clone()).await?;

        let banner = banners
            .or_filter(boards.contains(vec![board.id]))
            .or_filter(boards.is_null())
            .load::<Banner>(conn)
            .await?;

        use rand::seq::SliceRandom;
        Ok(banner
            .choose(&mut rand::thread_rng())
            .cloned()
            .ok_or_else(|| anyhow!("No banners found for board {}!", board_discriminator))?
            .safe())
    }

    pub async fn get_random_spoiler(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> Result<String> {
        use crate::schema::spoilers::dsl::*;
        use diesel::query_dsl::methods::SelectDsl;

        let spoiler = spoilers.select(img).load::<String>(conn).await?;

        use rand::seq::SliceRandom;
        spoiler
            .choose(&mut rand::thread_rng())
            .cloned()
            .ok_or_else(|| anyhow!("No spoilers found!"))
    }
}

pub fn replace_possible_profanity(mut string: String) -> String {
    let scrunkly = crate::PROFANITY.check_profanity(&string);
    for word in scrunkly {
        if word.category == profanity::Category::RacialSlurs
            || word.category_2 == Some(profanity::Category::RacialSlurs)
            || word.category_3 == Some(profanity::Category::RacialSlurs)
        {
            string = string.replace(&word.word, &crate::QUOTES.random_quote());
        }
    }
    string
}
